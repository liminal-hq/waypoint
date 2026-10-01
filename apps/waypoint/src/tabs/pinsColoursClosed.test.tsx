// Verifies pinned and coloured tabs, the tab and + menus, Recently Closed and the Ctrl+Tab switcher
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { stubLayout } from '../test/browseHarness';
import { DOCS, MUSIC, renderWorkspace } from '../test/workspaceHarness';
import { PLUS_HOLD_MS } from './TabStrip';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
	vi.useRealTimers();
});

type Harness = Awaited<ReturnType<typeof renderWorkspace>>;

const tabs = () => screen.getAllByRole('tab');
const names = () => tabs().map((tab) => tab.getAttribute('aria-label') ?? tab.textContent);
const snapshot = (h: Harness) => h.tabs.getSnapshot();
const live = () => screen.getAllByRole('status').find((el) => /srOnly/.test(el.className))!;

async function openTwo(h: Harness) {
	await h.tabs.openTab(DOCS);
	await h.tabs.openTab(MUSIC);
	await waitFor(() => expect(tabs()).toHaveLength(3));
}

async function openTabMenu(index: number) {
	fireEvent.contextMenu(tabs()[index]!);
	return screen.findByRole('menu', { name: 'Tab actions' });
}

const item = (name: string | RegExp) => screen.findByRole('menuitem', { name });

describe('pinned tabs', () => {
	it('are icon only, named for the folder, grouped first and have no close button', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await h.tabs.pinTab(3, true);
		await waitFor(() => expect(names()).toEqual(['music', 'test', 'docs']));
		const [pinned] = tabs();
		expect(pinned).toHaveAttribute('aria-label', 'music');
		expect(pinned).toHaveTextContent('');
		expect(pinned!.parentElement).toHaveAttribute('data-pinned');
		expect(pinned).toHaveAccessibleDescription('Pinned');
		expect(screen.queryByRole('button', { name: 'Close music' })).toBeNull();
		expect(screen.getByRole('button', { name: 'Close docs' })).toBeInTheDocument();
	});

	it('ignore a middle-click but still close from the keyboard and the menu', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await h.tabs.pinTab(2, true);
		await waitFor(() => expect(names()[0]).toBe('docs'));
		fireEvent(
			tabs()[0]!.parentElement!,
			new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true }),
		);
		expect(tabs()).toHaveLength(3);

		fireEvent.keyDown(tabs()[0]!, { key: 'Delete' });
		await waitFor(() => expect(tabs()).toHaveLength(2));
	});

	it('pin and unpin from the menu, which names the action for the tab', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await openTabMenu(1);
		fireEvent.click(await item('Pin Tab'));
		await waitFor(async () => expect((await snapshot(h)).tabs[0]!.id).toBe(2));
		expect((await snapshot(h)).tabs[0]!.pinned).toBe(true);
		await waitFor(() => expect(live()).toHaveTextContent('Pinned docs'));

		await openTabMenu(0);
		fireEvent.click(await item('Unpin Tab'));
		await waitFor(async () => expect((await snapshot(h)).tabs[0]!.pinned).toBe(false));
	});

	it('stay on their own side of the boundary for the keyboard reorder', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await h.tabs.pinTab(1, true);
		await waitFor(() => expect(tabs()[0]).toHaveAttribute('aria-label', 'test'));
		act(() => tabs()[0]!.focus());
		// The only pinned tab cannot move right past the unpinned ones.
		fireEvent.keyDown(tabs()[0]!, { key: 'ArrowRight', ctrlKey: true, shiftKey: true });
		// An unpinned tab cannot move left into the pinned block.
		act(() => tabs()[1]!.focus());
		fireEvent.keyDown(tabs()[1]!, { key: 'ArrowLeft', ctrlKey: true, shiftKey: true });
		expect((await snapshot(h)).tabs.map((tab) => tab.id)).toEqual([1, 2, 3]);
		fireEvent.keyDown(tabs()[1]!, { key: 'ArrowRight', ctrlKey: true, shiftKey: true });
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.map((tab) => tab.id)).toEqual([1, 3, 2]),
		);
	});

	it('clamp a drag at the pinned boundary', async () => {
		vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
			this: HTMLElement,
		) {
			const index = this.getAttribute('data-index');
			const left = index === null ? 0 : Number(index) * 100;
			return { left, right: left + 100, top: 0, bottom: 30, width: 100, height: 30 } as DOMRect;
		});
		const h = await renderWorkspace();
		await openTwo(h);
		await h.tabs.pinTab(1, true);
		await waitFor(() => expect(tabs()[0]).toHaveAttribute('aria-label', 'test'));
		const slot = tabs()[2]!.parentElement!;
		fireEvent.pointerDown(slot, { button: 0, clientX: 250, pointerId: 1 });
		fireEvent.pointerMove(slot, { clientX: 20, pointerId: 1 });
		fireEvent.pointerUp(slot, { clientX: 20, pointerId: 1 });
		// Dropped among the pinned tabs it lands first among the unpinned ones instead.
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.map((tab) => tab.id)).toEqual([1, 3, 2]),
		);
	});
});

describe('tab colours', () => {
	it('lists None and the palette, marks the current one and sets it', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await openTabMenu(1);
		fireEvent.click(await item('Colour'));
		const none = await screen.findByRole('menuitemcheckbox', { name: 'None' });
		expect(none).toHaveAttribute('aria-checked', 'true');
		const palette = screen.getAllByRole('menuitemcheckbox').map((entry) => entry.textContent);
		expect(palette).toEqual([
			'None',
			'Red',
			'Orange',
			'Yellow',
			'Green',
			'Teal',
			'Blue',
			'Purple',
			'Pink',
			'Grey',
		]);
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Teal' }));
		await waitFor(async () => expect((await snapshot(h)).tabs[1]!.colour).toBe('teal'));
		await waitFor(() => expect(live()).toHaveTextContent('Colour of docs set to Teal'));
	});

	it('shows an accent and says the colour in words', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await h.tabs.setTabColour(2, 'red');
		await h.tabs.pinTab(2, true);
		await waitFor(() => expect(tabs()[0]!.parentElement).toHaveAttribute('data-colour', 'red'));
		expect(tabs()[0]).toHaveAccessibleDescription('Pinned. Colour: Red');
		expect(tabs()[0]).toHaveAttribute('title', '/home/test/docs · Pinned · Colour: Red');
	});

	it('clears the colour with None', async () => {
		const h = await renderWorkspace();
		await h.tabs.setTabColour(1, 'blue');
		await waitFor(() => expect(tabs()[0]!.parentElement).toHaveAttribute('data-colour', 'blue'));
		await openTabMenu(0);
		fireEvent.click(await item('Colour'));
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: 'None' }));
		await waitFor(() => expect(tabs()[0]!.parentElement).not.toHaveAttribute('data-colour'));
	});
});

describe('the tab menu', () => {
	it('opens from the keyboard on a focused tab, with Menu or Shift+F10, and returns focus', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		act(() => tabs()[1]!.focus());
		fireEvent.keyDown(tabs()[1]!, { key: 'ContextMenu' });
		const menu = await screen.findByRole('menu', { name: 'Tab actions' });
		await waitFor(() =>
			expect(within(menu).getByRole('menuitem', { name: 'Pin Tab' })).toHaveFocus(),
		);
		fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		expect(tabs()[1]).toHaveFocus();

		fireEvent.keyDown(tabs()[1]!, { key: 'F10', shiftKey: true });
		expect(await screen.findByRole('menu', { name: 'Tab actions' })).toBeInTheDocument();
	});

	it('duplicates a tab beside itself', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await openTabMenu(1);
		fireEvent.click(await item('Duplicate Tab'));
		await waitFor(() => expect(tabs()).toHaveLength(4));
		expect(names()).toEqual(['test', 'docs', 'docs', 'music']);
	});

	it('closes this tab, the others, or the ones to the right, keeping pinned tabs', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await h.tabs.openTab(DOCS);
		await h.tabs.pinTab(1, true);
		await waitFor(() => expect(tabs()).toHaveLength(4));
		// Strip: test (pinned), docs, music, docs.
		await openTabMenu(2);
		fireEvent.click(await item('Close Tabs to the Right'));
		await waitFor(() => expect(tabs()).toHaveLength(3));
		expect(names()).toEqual(['test', 'docs', 'music']);

		await openTabMenu(2);
		fireEvent.click(await item('Close Other Tabs'));
		await waitFor(() => expect(tabs()).toHaveLength(2));
		expect(names()).toEqual(['test', 'music']);

		await openTabMenu(1);
		fireEvent.click(await item(/^Close Tab\s*Ctrl\+W$/));
		await waitFor(() => expect(tabs()).toHaveLength(1));
	});
});

describe('closed tabs', () => {
	it('reopens the most recently closed tab with Ctrl+Shift+T and announces it', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		fireEvent.click(screen.getByRole('button', { name: 'Close music' }));
		await waitFor(() => expect(tabs()).toHaveLength(2));
		fireEvent.keyDown(window, { key: 'T', ctrlKey: true, shiftKey: true });
		await waitFor(() => expect(tabs()).toHaveLength(3));
		expect(names()).toEqual(['test', 'docs', 'music']);
		expect(tabs()[2]).toHaveAttribute('aria-selected', 'true');
		await waitFor(() => expect(live()).toHaveTextContent('Reopened music'));
	});

	it('says so when there is nothing to reopen', async () => {
		await renderWorkspace();
		fireEvent.keyDown(window, { key: 'T', ctrlKey: true, shiftKey: true });
		await waitFor(() => expect(live()).toHaveTextContent('There are no closed tabs to reopen'));
	});

	it('lists the closed tabs from a fresh snapshot each time the + menu opens', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		const plus = screen.getByRole('button', { name: 'New tab' });
		fireEvent.contextMenu(plus);
		const first = await screen.findByRole('menu', { name: 'New tab actions' });
		expect(within(first).getByRole('menuitem', { name: /Reopen Closed Tab/ })).toHaveAttribute(
			'aria-disabled',
			'true',
		);
		fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());

		// Closing sends no event for the closed list, so only a fresh read shows it.
		fireEvent.click(screen.getByRole('button', { name: 'Close music' }));
		await waitFor(() => expect(tabs()).toHaveLength(2));
		const fresh = vi.spyOn(h.tabs, 'getSnapshot');
		fireEvent.contextMenu(plus);
		await screen.findByRole('menu', { name: 'New tab actions' });
		expect(fresh).toHaveBeenCalled();
		const reopen = screen.getByRole('menuitem', { name: /Reopen Closed Tab/ });
		expect(reopen).not.toHaveAttribute('aria-disabled');
		fireEvent.click(screen.getByRole('menuitem', { name: 'Recently Closed' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'music' }));
		await waitFor(() => expect(tabs()).toHaveLength(3));
		expect(names()[2]).toBe('music');
	});

	it('lists at most ten, newest first', async () => {
		const h = await renderWorkspace();
		for (let i = 0; i < 12; i++)
			await h.tabs.openTab({ ...DOCS, display: `/d/f${i}`, uri: `file:///d/f${i}` });
		await waitFor(() => expect(tabs()).toHaveLength(13));
		for (let i = 2; i <= 13; i++) await h.tabs.closeTab(i);
		await waitFor(() => expect(tabs()).toHaveLength(1));
		fireEvent.contextMenu(screen.getByRole('button', { name: 'New tab' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Recently Closed' }));
		const entries = await screen.findAllByRole('menuitem', { name: /^f\d+$/ });
		expect(entries).toHaveLength(10);
		expect(entries[0]).toHaveTextContent('f11');
	});

	it('is also in the tab menu', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		fireEvent.click(screen.getByRole('button', { name: 'Close music' }));
		await waitFor(() => expect(tabs()).toHaveLength(2));
		await openTabMenu(0);
		fireEvent.click(await item(/Reopen Closed Tab/));
		await waitFor(() => expect(tabs()).toHaveLength(3));
	});
});

describe('the + button menu', () => {
	const plus = () => screen.getByRole('button', { name: 'New tab' });

	it('opens on right-click with the new-tab, new-window and reopen items', async () => {
		await renderWorkspace();
		fireEvent.contextMenu(plus());
		const menu = await screen.findByRole('menu', { name: 'New tab actions' });
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((entry) => entry.textContent),
		).toEqual([
			'New TabCtrl+T',
			'New Tab at Home',
			'New WindowCtrl+Shift+N',
			'Reopen Closed TabCtrl+Shift+T',
			'Recently Closed',
		]);
	});

	it('opens on the Menu key and Shift+F10, focusing the first item', async () => {
		await renderWorkspace();
		act(() => plus().focus());
		fireEvent.keyDown(plus(), { key: 'ContextMenu' });
		const menu = await screen.findByRole('menu', { name: 'New tab actions' });
		await waitFor(() =>
			expect(within(menu).getByRole('menuitem', { name: /^New Tab\s*Ctrl/ })).toHaveFocus(),
		);
		fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		expect(plus()).toHaveFocus();
		fireEvent.keyDown(plus(), { key: 'F10', shiftKey: true });
		expect(await screen.findByRole('menu', { name: 'New tab actions' })).toBeInTheDocument();
	});

	it('opens on press-and-hold, without opening a tab when released', async () => {
		const h = await renderWorkspace();
		vi.useFakeTimers({ shouldAdvanceTime: true });
		fireEvent.pointerDown(plus(), { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
		act(() => {
			vi.advanceTimersByTime(PLUS_HOLD_MS + 10);
		});
		vi.useRealTimers();
		await screen.findByRole('menu', { name: 'New tab actions' });
		fireEvent.pointerUp(plus(), { pointerId: 1 });
		fireEvent.click(plus());
		expect((await snapshot(h)).tabs).toHaveLength(1);
	});

	it('does not open on a short press, which is a plain click', async () => {
		const h = await renderWorkspace();
		fireEvent.pointerDown(plus(), { button: 0, pointerId: 1 });
		fireEvent.pointerUp(plus(), { pointerId: 1 });
		fireEvent.click(plus());
		await waitFor(() => expect(tabs()).toHaveLength(2));
		expect(screen.queryByRole('menu')).toBeNull();
		expect((await snapshot(h)).tabs).toHaveLength(2);
	});

	it('opens a tab at Home from the menu', async () => {
		const h = await renderWorkspace();
		await h.tabs.navigate(1, DOCS);
		fireEvent.contextMenu(plus());
		fireEvent.click(await screen.findByRole('menuitem', { name: /New Tab at Home/ }));
		await waitFor(() => expect(tabs()).toHaveLength(2));
		expect((await snapshot(h)).tabs[1]!.location.display).toBe('/home/test');
	});
});

describe('the Ctrl+Tab switcher', () => {
	const switcher = () => screen.queryByTestId('tab-switcher');

	async function threeUsed(h: Harness) {
		await openTwo(h);
		// MRU: music, docs, test. Visit docs then music to leave music most recent.
		await h.tabs.activateTab(2);
		await h.tabs.activateTab(3);
		await waitFor(() => expect(tabs()[2]).toHaveAttribute('aria-selected', 'true'));
	}

	it('shows the candidate while Ctrl is held and commits once when it is released', async () => {
		const h = await renderWorkspace();
		await threeUsed(h);
		const activate = vi.spyOn(h.tabs, 'activateTab');
		fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
		fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
		expect(switcher()).not.toBeNull();
		// MRU order: music (active), docs, test; two presses reach test.
		const rows = within(switcher()!).getAllByRole('listitem', { hidden: true });
		await waitFor(() =>
			expect(rows.map((row) => row.textContent)).toEqual(['music', 'docs', 'test']),
		);
		expect(rows[2]).toHaveAttribute('data-candidate');
		expect(live()).toHaveTextContent('test, tab 1 of 3');
		expect(activate).not.toHaveBeenCalled();

		fireEvent.keyUp(window, { key: 'Control' });
		await waitFor(() => expect(tabs()[0]).toHaveAttribute('aria-selected', 'true'));
		expect(activate).toHaveBeenCalledTimes(1);
		expect(switcher()).toBeNull();
		// The tab passed on the way did not enter the MRU list.
		expect((await snapshot(h)).mru.slice(0, 3)).toEqual([1, 3, 2]);
	});

	it('ping-pongs between two tabs on quick presses', async () => {
		const h = await renderWorkspace();
		await threeUsed(h);
		for (let i = 0; i < 2; i++) {
			fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
			fireEvent.keyUp(window, { key: 'Control' });
		}
		await waitFor(() => expect(tabs()[2]).toHaveAttribute('aria-selected', 'true'));
		expect((await snapshot(h)).mru.slice(0, 2)).toEqual([3, 2]);
	});

	it('walks backwards with Shift', async () => {
		const h = await renderWorkspace();
		await threeUsed(h);
		fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true, shiftKey: true });
		fireEvent.keyUp(window, { key: 'Control' });
		await waitFor(() => expect(tabs()[0]).toHaveAttribute('aria-selected', 'true'));
	});

	it('is cancelled by Escape, leaving the active tab alone', async () => {
		const h = await renderWorkspace();
		await threeUsed(h);
		const activate = vi.spyOn(h.tabs, 'activateTab');
		fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(switcher()).toBeNull();
		fireEvent.keyUp(window, { key: 'Control' });
		expect(activate).not.toHaveBeenCalled();
		expect(live()).toHaveTextContent('Tab switch cancelled');
	});

	it('does nothing with a single tab', async () => {
		await renderWorkspace();
		fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
		expect(switcher()).toBeNull();
	});
});
