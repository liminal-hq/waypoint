// Verifies file drags in the whole browsing area: a row onto a folder row, a sidebar place, the Trash, a tab and across a pair
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { dismissNotice } from '../app/notices';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTrashClient } from '../trash/fakeTrashClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, DOCS, HOME, renderWorkspace } from '../test/workspaceHarness';
import { PLAN_REST_MS } from './fileDrag';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
	localStorage.clear();
	document.elementsFromPoint = () => [];
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
	// @ts-expect-error happy-dom has no hit test; the tests stand one in
	delete document.elementsFromPoint;
});

interface Mounted {
	ops: FakeOpsClient;
	tabs: FakeTabsApi;
}

async function mount(
	options: {
		paired?: boolean;
		sidebar?: boolean;
		trash?: boolean;
		dnd?: Partial<typeof DEFAULT_SETTINGS.dnd>;
	} = {},
) {
	const vfs = createTree();
	const tabs = new FakeTabsApi();
	await tabs.openTab(HOME);
	if (options.paired) {
		const second = await tabs.openTab(DOCS);
		const first = (await tabs.getSnapshot()).tabs[0]!.id;
		await tabs.joinPair([first, second], 'sideBySide');
		await tabs.activateTab(first);
	}
	const ops = createFakeOpsClient({
		resolveSelection: async (handle, spec) =>
			Promise.all(spec.ids.map((id) => vfs.entryLocation(handle, id))),
	});
	const places = new FakePlacesClient({ places: fakePlaces('/home/test') });
	await renderWorkspace(vfs, tabs, places, {
		sidebar: options.sidebar ?? false,
		ops,
		...(options.trash ? { trash: new FakeTrashClient() } : {}),
		...(options.dnd
			? {
					settings: createFakeSettingsClient({
						...DEFAULT_SETTINGS,
						dnd: { ...DEFAULT_SETTINGS.dnd, ...options.dnd },
					}),
				}
			: {}),
	});
	return { ops, tabs } satisfies Mounted;
}

const options = (pane = 0) =>
	within(screen.getAllByRole('listbox', { name: 'Files' })[pane]!).getAllByRole('option');
const row = (name: string, pane = 0) =>
	options(pane).find((option) => option.textContent?.includes(name))!;
const submits = (ops: FakeOpsClient) => ops.calls.filter((call) => call[0] === 'submit');
const lastSubmit = (ops: FakeOpsClient) => submits(ops).at(-1)![1] as Record<string, unknown>;

/** Presses on a row and drags 20 px: the drag has begun. */
async function grab(name: string, pane = 0) {
	await waitFor(() => expect(row(name, pane)).toBeDefined());
	fireEvent.pointerDown(row(name, pane), { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
	fireEvent.pointerMove(window, { pointerId: 1, clientX: 120, clientY: 100 });
}

/** What the hit test finds under the pointer from now on. */
const pointAt = (...elements: Element[]) => {
	document.elementsFromPoint = () => elements;
};

const moveTo = (
	x: number,
	keys: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean } = {},
) => fireEvent.pointerMove(window, { pointerId: 1, clientX: x, clientY: 100, ...keys });
const release = (
	x: number,
	keys: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean } = {},
) => fireEvent.pointerUp(window, { pointerId: 1, clientX: x, clientY: 100, ...keys });
const rest = () =>
	act(async () => void (await new Promise((done) => setTimeout(done, PLAN_REST_MS + 60))));
const pill = () => document.querySelector('[data-drag-pill]')?.textContent ?? null;

describe('dragging a row onto a folder row', () => {
	it('moves it, because the folder is on the same volume', async () => {
		const { ops } = await mount();
		await grab('notes.txt');
		expect(row('notes.txt')).toHaveAttribute('aria-selected', 'true');
		expect(pill()).toBe('Dragging notes.txtEsc to cancel');
		pointAt(row('docs'));
		moveTo(140);
		await waitFor(() => expect(pill()).toContain('Move or copy notes.txt to docs'));
		await rest();
		await waitFor(() => expect(pill()).toContain('Move notes.txt to docs'));
		expect(row('docs')).toHaveAttribute('data-drop-over', 'ok');
		release(140);
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'move' },
			sources: { kind: 'selection', spec: { kind: 'some', ids: [3] } },
			destination: { uri: 'file:///home/test/docs' },
			originWindow: 'main-1',
		});
		// The drag is over: nothing is marked, and the pill is gone.
		expect(row('docs')).not.toHaveAttribute('data-drop-over');
		expect(pill()).toBeNull();
	});

	it('copies when Ctrl is held at the release, whatever it was at the start', async () => {
		const { ops } = await mount();
		await grab('photo.jpg');
		pointAt(row('music'));
		moveTo(140);
		await rest();
		release(140, { ctrlKey: true });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'copy' },
			destination: { uri: 'file:///home/test/music' },
		});
	});

	it('drags the whole selection when one of its rows is dragged', async () => {
		const { ops } = await mount();
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		fireEvent.click(row('notes.txt'));
		fireEvent.click(row('photo.jpg'), { ctrlKey: true });
		await waitFor(() => expect(row('photo.jpg')).toHaveAttribute('aria-selected', 'true'));
		await grab('photo.jpg');
		pointAt(row('docs'));
		moveTo(140);
		await waitFor(() => expect(pill()).toContain('2 items to docs'));
		release(140, { shiftKey: true });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'move' },
			sources: { kind: 'selection', spec: { kind: 'some', ids: [3, 4] } },
		});
	});

	it('refuses to drop a folder on itself, and says why', async () => {
		const { ops } = await mount();
		await grab('docs');
		pointAt(row('docs'));
		moveTo(140);
		await waitFor(() => expect(pill()).toContain('Not allowed: docs is being dragged'));
		expect(row('docs')).toHaveAttribute('data-drop-over', 'blocked');
		release(140);
		await waitFor(() => expect(screen.getByText('Docs is being dragged')).toBeInTheDocument());
		expect(submits(ops)).toHaveLength(0);
	});

	it('is cancelled by Esc, which submits nothing and clears the marks', async () => {
		const { ops } = await mount();
		await grab('notes.txt');
		pointAt(row('docs'));
		moveTo(140);
		await waitFor(() => expect(row('docs')).toHaveAttribute('data-drop-over'));
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(row('docs')).not.toHaveAttribute('data-drop-over');
		expect(pill()).toBeNull();
		release(140);
		expect(submits(ops)).toHaveLength(0);
	});

	it('does not turn a click into a drag, nor a drag into a click', async () => {
		await mount();
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		fireEvent.pointerDown(row('notes.txt'), { pointerId: 1, button: 0, clientX: 5, clientY: 5 });
		fireEvent.pointerUp(window, { pointerId: 1, clientX: 5, clientY: 5 });
		fireEvent.click(row('notes.txt'));
		expect(row('notes.txt')).toHaveAttribute('aria-selected', 'true');
		// A drag that ends over another row leaves the selection alone: its click is swallowed.
		await grab('photo.jpg');
		pointAt(row('photo.jpg'));
		release(140);
		fireEvent.click(row('photo.jpg'));
		expect(row('notes.txt')).toHaveAttribute('aria-selected', 'false');
		expect(row('photo.jpg')).toHaveAttribute('aria-selected', 'true');
	});
});

describe('dragging onto the sidebar', () => {
	it('moves to a place, and to the Trash by moving it there', async () => {
		const { ops } = await mount({ sidebar: true, trash: true });
		const places = within(screen.getByRole('navigation', { name: 'Sidebar' })).getByRole('group', {
			name: 'Places',
		});
		await screen.findByRole('button', { name: /^Documents/ });
		await grab('notes.txt');
		pointAt(within(places).getByRole('button', { name: /^Documents/ }));
		moveTo(140);
		await rest();
		release(140);
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'move' },
			destination: { uri: 'file:///home/test/Documents' },
		});
		// Now the Trash.
		await grab('photo.jpg');
		pointAt(within(places).getByRole('button', { name: /^Trash/ }));
		moveTo(140);
		await waitFor(() => expect(pill()).toContain('Move photo.jpg to the Trash'));
		release(140);
		await waitFor(() => expect(submits(ops)).toHaveLength(2));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'trash' },
			sources: { kind: 'selection', spec: { kind: 'some', ids: [4] } },
		});
	});
});

describe('dragging across the panes of a pair', () => {
	it('drops into the other pane, which is a folder of its own', async () => {
		const { ops } = await mount({ paired: true });
		await waitFor(() => expect(screen.getAllByRole('listbox', { name: 'Files' })).toHaveLength(2));
		await grab('notes.txt', 0);
		const panes = document.querySelectorAll('[data-drop="pane"]');
		pointAt(panes[1]!);
		moveTo(300);
		await rest();
		await waitFor(() => expect(pill()).toContain('Move notes.txt to docs'));
		release(300);
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'move' },
			destination: { uri: 'file:///home/test/docs' },
		});
	});

	it('has no target over its own pane', async () => {
		const { ops } = await mount({ paired: true });
		await waitFor(() => expect(screen.getAllByRole('listbox', { name: 'Files' })).toHaveLength(2));
		await grab('notes.txt', 0);
		pointAt(document.querySelectorAll('[data-drop="pane"]')[0]!);
		moveTo(140);
		expect(pill()).toContain('Dragging notes.txt');
		release(140);
		expect(submits(ops)).toHaveLength(0);
	});
});

describe('the + button and the tabs', () => {
	it('opens the dropped folder in a new tab', async () => {
		const { tabs } = await mount();
		await grab('docs');
		pointAt(screen.getByRole('button', { name: 'New tab' }));
		moveTo(140);
		await waitFor(() => expect(pill()).toContain('Open in a new tab'));
		release(140);
		await waitFor(async () => expect((await tabs.getSnapshot()).tabs).toHaveLength(2));
		expect((await tabs.getSnapshot()).tabs[1]!.location.uri).toBe('file:///home/test/docs');
	});

	it('opens a file’s parent folder', async () => {
		const { tabs } = await mount();
		await grab('notes.txt');
		pointAt(screen.getByRole('button', { name: 'New tab' }));
		moveTo(140);
		release(140);
		await waitFor(async () => expect((await tabs.getSnapshot()).tabs).toHaveLength(2));
		expect((await tabs.getSnapshot()).tabs[1]!.location.uri).toBe(HOME.uri);
	});

	it('drops into the folder of another tab', async () => {
		const { ops, tabs } = await mount();
		await tabs.openTab(DOCS, { activate: false });
		await grab('notes.txt');
		const tab = await waitFor(() => {
			const found = document.querySelector('[data-drop="tab"][data-drop-label="docs"]');
			expect(found).not.toBeNull();
			return found!;
		});
		pointAt(tab);
		moveTo(140);
		await rest();
		release(140, { shiftKey: true });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'move' },
			destination: { uri: DOCS.uri },
		});
	});
});

describe('the settings', () => {
	it('always-copy copies on a drop with no modifier, and answers at once', async () => {
		const { ops } = await mount({ dnd: { defaultActionRule: 'alwaysCopy' } });
		await grab('notes.txt');
		pointAt(row('docs'));
		moveTo(140);
		await waitFor(() => expect(pill()).toContain('Copy notes.txt to docs'));
		release(140);
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({ kind: { kind: 'copy' } });
	});

	it('always-ask opens the picker on a drop with no modifier', async () => {
		await mount({ dnd: { defaultActionRule: 'alwaysAsk' } });
		await grab('notes.txt');
		pointAt(row('docs'));
		moveTo(140);
		release(140);
		await screen.findByRole('menu', { name: 'Drop action' });
	});

	it('spring-loads after the delay in the setting, and springs back on Esc with the selection kept', async () => {
		await mount({ dnd: { springLoadMs: 200 } });
		await grab('notes.txt');
		pointAt(row('docs'));
		moveTo(140);
		await waitFor(() => expect(row('docs')).toHaveAttribute('data-drop-spring'));
		expect(row('docs').style.getPropertyValue('--wp-drop-spring-ms')).toBe('200ms');
		// The pane opens the folder: its files are what the list shows now.
		await waitFor(() => expect(row('report.pdf')).toBeDefined(), { timeout: 1500 });
		fireEvent.keyDown(window, { key: 'Escape' });
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		// The very session came back: the dragged row is still the selection.
		expect(row('notes.txt')).toHaveAttribute('aria-selected', 'true');
	});
});

describe('the action picker', () => {
	it('opens on a right-button drag and runs the choice', async () => {
		const { ops } = await mount();
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		fireEvent.pointerDown(row('notes.txt'), {
			pointerId: 1,
			button: 2,
			clientX: 100,
			clientY: 100,
		});
		fireEvent.pointerMove(window, { pointerId: 1, clientX: 120, clientY: 100 });
		pointAt(row('docs'));
		moveTo(140);
		await waitFor(() => expect(pill()).toContain('Choose what to do with notes.txt in docs'));
		release(140);
		const menu = await screen.findByRole('menu', { name: 'Drop action' });
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((item) => item.textContent),
		).toEqual(['Copy Here', 'Move Here', 'Link Here', 'Cancel']);
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Link Here' }));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'link' },
			destination: { uri: 'file:///home/test/docs' },
		});
	});

	it('shows the entry menu for a right click that is not a drag, once the button is released', async () => {
		await mount();
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		fireEvent.pointerDown(row('notes.txt'), {
			pointerId: 1,
			button: 2,
			clientX: 100,
			clientY: 100,
		});
		fireEvent.contextMenu(row('notes.txt'), { clientX: 100, clientY: 100 });
		// Held back while the press may still become a drag.
		expect(screen.queryByRole('menu')).toBeNull();
		fireEvent.pointerUp(window, { pointerId: 1, clientX: 100, clientY: 100 });
		await screen.findByRole('menu');
	});

	it('shows the entry menu at once for a right click with no press before it', async () => {
		await mount();
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		fireEvent.contextMenu(row('notes.txt'), { clientX: 100, clientY: 100 });
		await screen.findByRole('menu');
	});
});

describe('a tab drag', () => {
	it('is not a file drag, and a file drag does not start while it runs', async () => {
		const { tabs } = await mount();
		await tabs.openTab(DOCS);
		await waitFor(() => expect(document.querySelectorAll('[role="tab"]').length).toBe(2));
		const tab = document.querySelectorAll('[data-slot]')[0]!;
		fireEvent.pointerDown(tab, { pointerId: 5, button: 0, clientX: 40, clientY: 10 });
		fireEvent.pointerMove(window, { pointerId: 5, clientX: 80, clientY: 10 });
		await waitFor(() => expect(document.querySelector('[data-drag-pill]')).not.toBeNull());
		// The only pill is the tab's.
		expect(document.querySelectorAll('[data-drag-pill]')).toHaveLength(1);
		expect(pill()).toContain('Release to');
		fireEvent.pointerUp(window, { pointerId: 5, clientX: 80, clientY: 10 });
	});
});
