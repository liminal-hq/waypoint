// Verifies new windows and the hand-off of tabs between windows: keys, gestures, menus, notices and announcements
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { fileLocation } from '../services/fakeVfsClient';
import { MAX_WINDOWS } from '../services/tabsApi';
import { stubLayout } from '../test/browseHarness';
import { DOCS, HOME, MUSIC, renderWorkspace } from '../test/workspaceHarness';
import { createWindowActions, handleWindowKey } from './windowActions';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

/** Two windows over one store, as the app's policy has it: a window closes with its last tab. */
function twoWindows() {
	const store = new FakeTabsStore({ policy: { closeWindowOnLastTab: true } });
	const one = new FakeTabsApi(store, 'main-1');
	return { store, one };
}

const keyNewWindow = () =>
	fireEvent.keyDown(window, { key: 'N', ctrlKey: true, shiftKey: true, cancelable: true });
const auxClick = (element: Element, init: MouseEventInit = {}) =>
	fireEvent(
		element,
		new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true, ...init }),
	);
const option = (name: string) => screen.findByRole('option', { name: new RegExp(`^${name}`) });
const tabs = () => screen.getAllByRole('tab');
const liveRegion = () =>
	screen.getAllByRole('status').find((el) => el.className.includes('srOnly'));

describe('a new window', () => {
	it('opens with Ctrl+Shift+N, holding one tab at Home', async () => {
		const { store, one } = twoWindows();
		await renderWorkspace(undefined, one);
		keyNewWindow();
		await waitFor(() => expect(store.windowLabels()).toEqual(['main-1', 'main-2']));
		const added = store.window('main-2')!;
		expect(added.tabs.map((tab) => tab.location.uri)).toEqual([HOME.uri]);
		// The window the key was pressed in keeps its own tab.
		expect(store.window('main-1')!.tabs).toHaveLength(1);
	});

	it('is the key the handler answers to, and nothing else', () => {
		const actions = { newWindow: vi.fn(async () => {}) };
		const key = { key: 'n', ctrlKey: true, metaKey: false, altKey: false, shiftKey: true };
		expect(handleWindowKey(key, actions)).toBe(true);
		expect(handleWindowKey({ ...key, shiftKey: false }, actions)).toBe(false);
		expect(handleWindowKey({ ...key, key: 't' }, actions)).toBe(false);
		expect(actions.newWindow).toHaveBeenCalledTimes(1);
	});

	it('opens a folder with Ctrl+middle-click, and a plain middle-click still opens a tab', async () => {
		const { store, one } = twoWindows();
		await renderWorkspace(undefined, one);
		auxClick(await option('docs'));
		await waitFor(() => expect(tabs()).toHaveLength(2));
		expect(store.windowLabels()).toEqual(['main-1']);

		auxClick(await option('music'), { ctrlKey: true });
		await waitFor(() => expect(store.windowLabels()).toEqual(['main-1', 'main-2']));
		expect(store.window('main-2')!.tabs.map((tab) => tab.location.uri)).toEqual([MUSIC.uri]);
		expect(tabs()).toHaveLength(2);
	});

	it('warns from the eighth window and refuses the thirteenth with a notice', async () => {
		const { store, one } = twoWindows();
		await renderWorkspace(undefined, one);
		for (let n = 2; n < 8; n += 1) await one.openWindow(HOME);
		expect(store.windowLabels()).toHaveLength(7);
		keyNewWindow();
		const warning = await screen.findByRole('alert');
		expect(warning).toHaveTextContent('Many windows are open');

		for (let n = store.windowLabels().length; n < MAX_WINDOWS; n += 1) await one.openWindow(HOME);
		expect(store.windowLabels()).toHaveLength(MAX_WINDOWS);
		keyNewWindow();
		await waitFor(() =>
			expect(screen.getByRole('alert')).toHaveTextContent(
				'Waypoint cannot open more than 12 windows. Close one first.',
			),
		);
		expect(store.windowLabels()).toHaveLength(MAX_WINDOWS);
	});
});

describe('the tab menu', () => {
	it('lists the other windows by their folder and tab count, with the submenu roles', async () => {
		const { store, one } = twoWindows();
		const two = new FakeTabsApi(store, 'main-2');
		await two.openTab(DOCS);
		await two.openTab(MUSIC);
		await renderWorkspace(undefined, one);
		fireEvent.contextMenu(tabs()[0]!, { clientX: 10, clientY: 10 });
		const menu = await screen.findByRole('menu', { name: 'Tab actions' });
		const labels = within(menu)
			.getAllByRole('menuitem')
			.map((item) => item.textContent);
		expect(labels).toContain('Move to New Window');
		expect(labels).toContain('Move to Window');
		const submenu = within(menu).getByRole('menuitem', { name: 'Move to Window' });
		expect(submenu).toHaveAttribute('aria-haspopup', 'menu');
		fireEvent.click(submenu);
		const entries = await screen.findAllByRole('menuitem', { name: /— 2 tabs$/ });
		expect(entries.map((item) => item.textContent)).toEqual(['music — 2 tabs']);
	});

	it('disables Move to Window when there is no other window', async () => {
		const { one } = twoWindows();
		await renderWorkspace(undefined, one);
		fireEvent.contextMenu(tabs()[0]!, { clientX: 10, clientY: 10 });
		const menu = await screen.findByRole('menu', { name: 'Tab actions' });
		expect(within(menu).getByRole('menuitem', { name: 'Move to Window' })).toHaveAttribute(
			'aria-disabled',
			'true',
		);
	});

	it('opens from the keyboard with the menu key and focuses into it', async () => {
		const { one } = twoWindows();
		await renderWorkspace(undefined, one);
		const tab = tabs()[0]!;
		tab.focus();
		fireEvent.keyDown(tab, { key: 'ContextMenu' });
		const menu = await screen.findByRole('menu', { name: 'Tab actions' });
		await waitFor(() => expect(menu.contains(document.activeElement)).toBe(true));
		fireEvent.keyDown(menu, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		expect(tab).toHaveFocus();
	});

	it('moves a tab to a new window and says so; the last tab takes its window with it', async () => {
		const { store, one } = twoWindows();
		await renderWorkspace(undefined, one);
		await one.openTab(DOCS);
		await waitFor(() => expect(tabs()).toHaveLength(2));
		const docs = (await one.getSnapshot()).tabs[1]!;

		fireEvent.contextMenu(tabs()[1]!, { clientX: 10, clientY: 10 });
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Move to New Window' }));
		await waitFor(() => expect(store.windowLabels()).toEqual(['main-1', 'main-2']));
		// The tab keeps its id, and a hand-off is not a close.
		expect(store.window('main-2')!.tabs.map((tab) => tab.id)).toEqual([docs.id]);
		expect(store.closed()).toEqual([]);
		await waitFor(() => expect(tabs()).toHaveLength(1));
		await waitFor(() => expect(liveRegion()).toHaveTextContent('Moved to a new window'));

		// Moving the only tab left in main-1 closes main-1; the tab is not recorded as closed.
		const actions = createWindowActions(one, HOME, async () => {});
		await actions.moveToNewWindow((await one.getSnapshot()).tabs[0]!);
		expect(store.windowLabels()).toEqual(['main-2', 'main-3']);
		expect(store.closed()).toEqual([]);
	});

	it('moves a tab to another window and the target announces it', async () => {
		const { store, one } = twoWindows();
		const two = new FakeTabsApi(store, 'main-2');
		await two.openTab(MUSIC);
		await renderWorkspace(undefined, one);
		await one.openTab(DOCS);
		await waitFor(() => expect(tabs()).toHaveLength(2));

		// The window that receives the tab is the one rendered here: hand a tab to it from main-2.
		await two.openTab(fileLocation('/home/test/music/albums'));
		const arriving = (await two.getSnapshot()).tabs[1]!;
		// Straight to the command: the actions' own announcement belongs to the sending window.
		await two.moveTabs(
			{ kind: 'tabs', value: [arriving.id] },
			{ kind: 'existingWindow', label: 'main-1', index: 2 },
		);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		await waitFor(() => expect(liveRegion()).toHaveTextContent('Moved albums to this window'));
		expect(store.window('main-1')!.tabs.at(-1)!.id).toBe(arriving.id);
	});
});

describe('the hand-off', () => {
	it('reports the tab’s hints before it moves, and does not move it if that never settles first', async () => {
		const { store, one } = twoWindows();
		const id = await one.openTab(DOCS);
		await one.openTab(MUSIC);
		const order: string[] = [];
		let release: () => void = () => {};
		const flush = vi.fn(
			() =>
				new Promise<void>((resolve) => {
					release = () => {
						order.push('flushed');
						resolve();
					};
				}),
		);
		const moveTabs = one.moveTabs.bind(one);
		one.moveTabs = async (...args) => {
			order.push('moved');
			return moveTabs(...args);
		};
		const actions = createWindowActions(one, HOME, flush);
		const tab = (await one.getSnapshot()).tabs.find((candidate) => candidate.id === id)!;
		const done = actions.moveToNewWindow(tab);
		await Promise.resolve();
		expect(flush).toHaveBeenCalledWith(id);
		expect(store.windowLabels()).toEqual(['main-1']);
		release();
		await done;
		expect(order).toEqual(['flushed', 'moved']);
		expect(store.windowLabels()).toEqual(['main-1', 'main-2']);
	});

	it('still moves the tab when reporting its hints fails', async () => {
		const { store, one } = twoWindows();
		const id = await one.openTab(DOCS);
		await one.openTab(MUSIC);
		const actions = createWindowActions(one, HOME, () => Promise.resolve());
		await actions.moveToNewWindow((await one.getSnapshot()).tabs.find((tab) => tab.id === id)!);
		expect(store.windowLabels()).toHaveLength(2);
	});

	it('tells the window the tabs went to, and only an existing one', async () => {
		const { store, one } = twoWindows();
		const a = await one.openTab(DOCS);
		await one.openTab(MUSIC);
		const two = new FakeTabsApi(store, 'main-2');
		await two.openTab(HOME);
		const told = vi.fn();
		two.onHandoff(told);
		await one.moveTabs(
			{ kind: 'tabs', value: [a] },
			{ kind: 'existingWindow', label: 'main-2', index: 1 },
		);
		expect(told).toHaveBeenCalledWith({ tabs: [a], from: 'main-1' });
		const quiet = vi.fn();
		two.onHandoff(quiet);
		await two.moveTabs(
			{ kind: 'tabs', value: [a] },
			{ kind: 'newWindow', label: null, geometry: null },
		);
		expect(quiet).not.toHaveBeenCalled();
	});

	it('announces several arriving tabs together', async () => {
		const { store, one } = twoWindows();
		const two = new FakeTabsApi(store, 'main-2');
		await renderWorkspace(undefined, two);
		const a = await one.openTab(DOCS);
		const b = await one.openTab(MUSIC);
		await one.openTab(HOME);
		await one.moveTabs(
			{ kind: 'tabs', value: [a, b] },
			{ kind: 'existingWindow', label: 'main-2', index: 1 },
		);
		await waitFor(() => expect(liveRegion()).toHaveTextContent('Moved 2 tabs to this window'));
	});
});
