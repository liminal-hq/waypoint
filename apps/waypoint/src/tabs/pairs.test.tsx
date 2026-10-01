// Verifies pairs in the browsing area: panes, dividers, the pill, F3 and F6, the joint menu and closing a half
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NOTICE_TOAST_MS } from '../app/NoticeToast';
import { dismissNotice } from '../app/notices';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { stubLayout } from '../test/browseHarness';
import { DOCS, MUSIC, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
	vi.restoreAllMocks();
	vi.useRealTimers();
});

type Harness = Awaited<ReturnType<typeof renderWorkspace>>;

const tabs = () => screen.getAllByRole('tab');
const panes = () => screen.queryAllByRole('group', { name: /^Pane \d+ of \d+/ });
const snapshot = (h: Harness) => h.tabs.getSnapshot();
const live = () => screen.getAllByRole('status').find((el) => /srOnly/.test(el.className))!;
const divider = () => screen.getByRole('separator', { name: 'Resize panes' });
const joint = () => screen.getByRole('button', { name: /^Split: / });
const item = (name: string | RegExp) => screen.findByRole('menuitem', { name });

async function split() {
	fireEvent.keyDown(window, { key: 'F3' });
	await waitFor(() => expect(panes()).toHaveLength(2));
}

/** Two tabs (home and docs) joined by hand. */
async function joined(h: Harness, layout: 'sideBySide' | 'stacked' = 'sideBySide') {
	await h.tabs.openTab(DOCS, { activate: false });
	await h.tabs.joinPair([1, 2], layout);
	await waitFor(() => expect(panes()).toHaveLength(2));
}

describe('a single tab', () => {
	it('shows no pane header, divider or joint, as before', async () => {
		await renderWorkspace();
		await screen.findAllByRole('option');
		expect(panes()).toHaveLength(0);
		expect(screen.queryByRole('separator')).toBeNull();
		expect(screen.queryByRole('button', { name: /^Split: / })).toBeNull();
	});
});

describe('the pane layout', () => {
	it('shows each pane of the active pair over a live listing of its own', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await waitFor(() => expect(h.client.openCount).toBe(2));
		const [first, second] = panes();
		expect(first).toHaveAccessibleName('Pane 1 of 2: test');
		expect(second).toHaveAccessibleName('Pane 2 of 2: docs');
		expect(within(first!).getByText('notes.txt')).toBeInTheDocument();
		expect(await within(second!).findByText('report.pdf')).toBeInTheDocument();
		// Only the active pane carries the active outline and says so.
		expect(first).toHaveAttribute('data-active');
		expect(second).not.toHaveAttribute('data-active');
		expect(within(first!).getByText('Active')).toBeInTheDocument();
	});

	it('follows the layout and the sizes of the pair', async () => {
		const h = await renderWorkspace();
		await joined(h);
		const area = panes()[0]!.parentElement!;
		expect(area).toHaveAttribute('data-layout', 'sideBySide');
		expect(divider()).toHaveAttribute('aria-orientation', 'vertical');
		await act(async () => {
			await h.tabs.setPairLayout(1, 'stacked');
			await h.tabs.setPairSizes(1, [300, 700]);
		});
		await waitFor(() => expect(area).toHaveAttribute('data-layout', 'stacked'));
		expect(divider()).toHaveAttribute('aria-orientation', 'horizontal');
		await waitFor(() => expect(panes()[0]).toHaveStyle({ flexGrow: '300' }));
		expect(panes()[1]).toHaveStyle({ flexGrow: '700' });
		expect(divider()).toHaveAttribute('aria-valuenow', '30');
	});

	it('goes back to one pane without a header when the pair separates', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await act(async () => {
			await h.tabs.separatePair(1);
		});
		await waitFor(() => expect(panes()).toHaveLength(0));
		expect(screen.queryByRole('separator')).toBeNull();
	});

	it('shows a pair that has three panes with two dividers', async () => {
		const h = await renderWorkspace();
		await h.tabs.openTab(DOCS);
		await h.tabs.openTab(MUSIC);
		await h.tabs.joinPair([1, 2, 3], 'sideBySide');
		await waitFor(() => expect(panes()).toHaveLength(3));
		expect(screen.getAllByRole('separator')).toHaveLength(2);
	});
});

describe('the divider', () => {
	it('moves with the arrow keys along the layout and writes the sizes', async () => {
		const h = await renderWorkspace();
		await joined(h);
		expect(divider()).toHaveAttribute('aria-valuenow', '50');
		fireEvent.keyDown(divider(), { key: 'ArrowRight' });
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.sizes).toEqual([520, 480]));
		await waitFor(() => expect(divider()).toHaveAttribute('aria-valuenow', '52'));
		fireEvent.keyDown(divider(), { key: 'ArrowLeft', shiftKey: true });
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.sizes).toEqual([420, 580]));
		// Up and Down do nothing for a side-by-side pair.
		fireEvent.keyDown(divider(), { key: 'ArrowUp' });
		expect((await snapshot(h)).pairs[0]!.sizes).toEqual([420, 580]);
	});

	it('uses Up and Down when stacked, and Enter resets', async () => {
		const h = await renderWorkspace();
		await joined(h, 'stacked');
		fireEvent.keyDown(divider(), { key: 'ArrowDown' });
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.sizes).toEqual([520, 480]));
		fireEvent.keyDown(divider(), { key: 'Enter' });
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.sizes).toEqual([500, 500]));
	});

	it('never takes a pane under a tenth', async () => {
		const h = await renderWorkspace();
		await joined(h);
		for (let i = 0; i < 12; i++)
			fireEvent.keyDown(divider(), { key: 'ArrowRight', shiftKey: true });
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.sizes).toEqual([900, 100]));
		expect(divider()).toHaveAttribute('aria-valuemax', '90');
	});

	it('resets to equal sizes on double-click', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await act(async () => {
			await h.tabs.setPairSizes(1, [250, 750]);
		});
		await waitFor(() => expect(divider()).toHaveAttribute('aria-valuenow', '25'));
		fireEvent.doubleClick(divider());
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.sizes).toEqual([500, 500]));
	});

	describe('dragged with the pointer', () => {
		function stubArea() {
			vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
				this: HTMLElement,
			) {
				const separator = this.getAttribute('role') === 'separator';
				const width = separator ? 5 : this.hasAttribute('data-layout') ? 1005 : 0;
				return { left: 0, right: width, top: 0, bottom: 0, width, height: 0 } as DOMRect;
			});
		}

		it('resizes live and writes the sizes once, on release', async () => {
			stubArea();
			const h = await renderWorkspace();
			await joined(h);
			const write = vi.spyOn(h.tabs, 'setPairSizes');
			fireEvent.pointerDown(divider(), { button: 0, clientX: 500, pointerId: 1 });
			fireEvent.pointerMove(divider(), { clientX: 600, pointerId: 1 });
			// The area is 1000 px of panes, so 100 px is a tenth: 600 / 400.
			await waitFor(() => expect(panes()[0]).toHaveStyle({ flexGrow: '600' }));
			expect(panes()[1]).toHaveStyle({ flexGrow: '400' });
			expect(write).not.toHaveBeenCalled();
			fireEvent.pointerUp(divider(), { clientX: 600, pointerId: 1 });
			await waitFor(() => expect(write).toHaveBeenCalledWith(1, [600, 400]));
			expect(write).toHaveBeenCalledTimes(1);
			await waitFor(async () => expect((await snapshot(h)).pairs[0]!.sizes).toEqual([600, 400]));
			expect(panes()[0]).toHaveStyle({ flexGrow: '600' });
		});

		it('is abandoned with Escape, and writes nothing', async () => {
			stubArea();
			const h = await renderWorkspace();
			await joined(h);
			const write = vi.spyOn(h.tabs, 'setPairSizes');
			fireEvent.pointerDown(divider(), { button: 0, clientX: 500, pointerId: 1 });
			fireEvent.pointerMove(divider(), { clientX: 700, pointerId: 1 });
			await waitFor(() => expect(panes()[0]).toHaveStyle({ flexGrow: '700' }));
			fireEvent.keyDown(window, { key: 'Escape' });
			await waitFor(() => expect(panes()[0]).toHaveStyle({ flexGrow: '500' }));
			fireEvent.pointerUp(divider(), { clientX: 700, pointerId: 1 });
			expect(write).not.toHaveBeenCalled();
		});
	});
});

describe('the focused pane', () => {
	it('is followed by the path bar and the tab strip', async () => {
		const h = await renderWorkspace();
		await joined(h);
		const crumbs = () => screen.getByRole('navigation', { name: 'Location' });
		await waitFor(() => expect(within(crumbs()).getByText('test')).toBeInTheDocument());
		fireEvent.pointerDown(panes()[1]!);
		await waitFor(() => expect(within(crumbs()).getByText('docs')).toBeInTheDocument());
		expect(await snapshot(h)).toMatchObject({ active: 2 });
	});

	it('activates a pane when it is pressed or focused', async () => {
		const h = await renderWorkspace();
		await joined(h);
		fireEvent.pointerDown(panes()[1]!);
		await waitFor(async () => expect((await snapshot(h)).active).toBe(2));
		await waitFor(() => expect(panes()[1]).toHaveAttribute('data-active'));
		expect(panes()[0]).not.toHaveAttribute('data-active');
		// The tab strip shows it too.
		expect(tabs()[1]).toHaveAttribute('aria-selected', 'true');
		fireEvent.focus(within(panes()[0]!).getAllByRole('listbox')[0]!);
		await waitFor(async () => expect((await snapshot(h)).active).toBe(1));
	});

	it('moves with F6 and Shift+F6, wrapping, and says which pane', async () => {
		const h = await renderWorkspace();
		await joined(h);
		fireEvent.keyDown(window, { key: 'F6' });
		await waitFor(async () => expect((await snapshot(h)).active).toBe(2));
		await waitFor(() => expect(live()).toHaveTextContent('Pane 2 of 2: docs'));
		fireEvent.keyDown(window, { key: 'F6' });
		await waitFor(async () => expect((await snapshot(h)).active).toBe(1));
		fireEvent.keyDown(window, { key: 'F6', shiftKey: true });
		await waitFor(async () => expect((await snapshot(h)).active).toBe(2));
	});

	it('puts keyboard focus in the pane that F6 chose', async () => {
		const h = await renderWorkspace();
		await joined(h);
		fireEvent.keyDown(window, { key: 'F6' });
		await waitFor(() => {
			const list = within(panes()[1]!).getByRole('listbox');
			expect(document.activeElement).toBe(list);
		});
	});

	it('leaves F6 alone on a tab that is not paired', async () => {
		await renderWorkspace();
		const event = new KeyboardEvent('keydown', { key: 'F6', cancelable: true, bubbles: true });
		window.dispatchEvent(event);
		expect(event.defaultPrevented).toBe(false);
	});

	it('navigates the pane the folder was opened in', async () => {
		const h = await renderWorkspace();
		await joined(h);
		const second = panes()[1]!;
		// Opening a folder in the inactive pane moves that pane's tab, not the active one.
		const folder = await within(panes()[0]!).findByText('music');
		fireEvent.doubleClick(folder);
		await waitFor(async () =>
			expect((await snapshot(h)).tabs[0]!.location.display).toContain('music'),
		);
		expect((await snapshot(h)).tabs[1]!.location.display).toContain('docs');
		expect(second).toBeInTheDocument();
	});
});

describe('the pair pill', () => {
	it('names the split and joins the halves while each stays a tab', async () => {
		const h = await renderWorkspace();
		await joined(h);
		expect(tabs()).toHaveLength(2);
		expect(joint()).toHaveAccessibleName('Split: test and docs');
		expect(tabs()[0]).toHaveAccessibleDescription('Split: test and docs');
		expect(tabs()[1]).toHaveAccessibleDescription('Split: test and docs');
		expect(tabs()[0]!.parentElement).toHaveAttribute('data-pair-end', 'start');
		expect(tabs()[1]!.parentElement).toHaveAttribute('data-pair-end', 'end');
		// The seam is on the first half only.
		expect(screen.getAllByRole('button', { name: /^Split: / })).toHaveLength(1);
	});

	it('activates the half that is clicked, and the roving focus treats the halves separately', async () => {
		const h = await renderWorkspace();
		await joined(h);
		fireEvent.click(tabs()[1]!.parentElement!);
		await waitFor(async () => expect((await snapshot(h)).active).toBe(2));
		tabs()[1]!.focus();
		fireEvent.keyDown(tabs()[1]!, { key: 'ArrowLeft' });
		expect(document.activeElement).toBe(tabs()[0]);
		expect(tabs()[0]).toHaveAttribute('tabindex', '0');
		expect(tabs()[1]).toHaveAttribute('tabindex', '-1');
	});

	it('does nothing on a plain click of the joint', async () => {
		const h = await renderWorkspace();
		await joined(h);
		const before = await snapshot(h);
		fireEvent.click(joint());
		fireEvent.pointerDown(joint(), { button: 0 });
		expect(screen.queryByRole('menu')).toBeNull();
		expect(await snapshot(h)).toEqual(before);
	});

	it('resets the pane sizes when the joint is double-clicked', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await act(async () => {
			await h.tabs.setPairSizes(1, [250, 750]);
		});
		fireEvent.doubleClick(joint());
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.sizes).toEqual([500, 500]));
	});

	it('is pinned and coloured as one', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await h.tabs.pinTab(2, true);
		await waitFor(() => expect(tabs()[0]!.parentElement).toHaveAttribute('data-pinned'));
		expect(tabs()[1]!.parentElement).toHaveAttribute('data-pinned');
		expect((await snapshot(h)).pairs).toHaveLength(1);
	});
});

describe('the joint menu', () => {
	async function openJointMenu() {
		fireEvent.contextMenu(joint());
		return screen.findByRole('menu', { name: 'Split actions' });
	}

	it('lists what D30 asks for, without the parked items', async () => {
		const h = await renderWorkspace();
		await joined(h);
		const menu = await openJointMenu();
		const names = within(menu)
			.getAllByRole('menuitem')
			.map((element) => element.textContent);
		expect(names).toEqual([
			'Separate',
			'Swap Panes',
			'Layout',
			'Reset Sizes',
			'Pin Tab',
			'Colour',
			'Duplicate Split',
			'Move to New Window',
			'Close Both',
		]);
		expect(screen.queryByRole('menuitem', { name: /Sync Navigation|Compare Folders/ })).toBeNull();
	});

	it('opens from the keyboard on the focused joint', async () => {
		const h = await renderWorkspace();
		await joined(h);
		joint().focus();
		fireEvent.keyDown(joint(), { key: 'F10', shiftKey: true });
		expect(await screen.findByRole('menu', { name: 'Split actions' })).toBeInTheDocument();
		fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
	});

	it('swaps the panes', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await openJointMenu();
		fireEvent.click(await item('Swap Panes'));
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.panes).toEqual([2, 1]));
		await waitFor(() => expect(live()).toHaveTextContent('Swapped the panes'));
		await waitFor(() => expect(panes()[0]).toHaveAccessibleName('Pane 1 of 2: docs'));
	});

	it('changes the layout and marks the current one', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await openJointMenu();
		fireEvent.click(await item('Layout'));
		expect(await screen.findByRole('menuitemcheckbox', { name: 'Side by Side' })).toHaveAttribute(
			'aria-checked',
			'true',
		);
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Stacked' }));
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.layout).toBe('stacked'));
		await waitFor(() => expect(live()).toHaveTextContent('Panes are now Stacked'));
	});

	it('resets the sizes', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await act(async () => {
			await h.tabs.setPairSizes(1, [250, 750]);
		});
		await openJointMenu();
		fireEvent.click(await item('Reset Sizes'));
		await waitFor(async () => expect((await snapshot(h)).pairs[0]!.sizes).toEqual([500, 500]));
	});

	it('separates, leaving two single tabs', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await openJointMenu();
		fireEvent.click(await item('Separate'));
		await waitFor(async () => expect((await snapshot(h)).pairs).toEqual([]));
		expect((await snapshot(h)).tabs).toHaveLength(2);
		await waitFor(() => expect(live()).toHaveTextContent('Separated test and docs'));
	});

	it('pins and unpins the pair, and colours both tabs', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await openJointMenu();
		fireEvent.click(await item('Pin Tab'));
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.every((tab) => tab.pinned)).toBe(true),
		);
		await waitFor(() => expect(live()).toHaveTextContent('Pinned test and docs'));
		fireEvent.contextMenu(joint());
		expect(await item('Unpin Tab')).toBeInTheDocument();
		fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());

		fireEvent.contextMenu(joint());
		fireEvent.click(await item('Colour'));
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: 'Teal' }));
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.map((tab) => tab.colour)).toEqual(['teal', 'teal']),
		);
	});

	it('duplicates both panes as a new pair', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await openJointMenu();
		fireEvent.click(await item('Duplicate Split'));
		await waitFor(async () => expect((await snapshot(h)).pairs).toHaveLength(2));
		const state = await snapshot(h);
		expect(state.tabs).toHaveLength(4);
		expect(state.tabs.map((tab) => tab.location.display.split('/').pop())).toEqual([
			'test',
			'docs',
			'test',
			'docs',
		]);
	});

	it('closes both tabs, and closes the window when they were all there was', async () => {
		const store = new FakeTabsStore({ policy: { closeWindowOnLastTab: true } });
		const h = await renderWorkspace(undefined, new FakeTabsApi(store, 'main-1'));
		const other = new FakeTabsApi(store, 'main-2');
		await joined(h);
		await other.openTab(MUSIC);
		await openJointMenu();
		fireEvent.click(await item('Close Both'));
		await waitFor(() => expect(store.windowLabels()).toEqual(['main-2']));
		// No Home tab opened on the way out; both tabs are in Recently Closed.
		expect(store.window('main-2')!.tabs).toHaveLength(1);
		expect(store.closed()).toHaveLength(2);
	});

	it('says so, and changes nothing, when the new window cannot be made', async () => {
		const h = await renderWorkspace();
		await joined(h);
		const move = vi
			.spyOn(h.tabs, 'moveTabs')
			.mockRejectedValue('could not create the window: creating windows is not available yet');
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		await openJointMenu();
		fireEvent.click(await item('Move to New Window'));
		await waitFor(() =>
			expect(move).toHaveBeenCalledWith(
				{ kind: 'pair', value: 1 },
				{ kind: 'newWindow', label: null, geometry: null },
			),
		);
		expect(
			await screen.findByText('The split could not be moved to a new window.'),
		).toBeInTheDocument();
		expect((await snapshot(h)).pairs).toHaveLength(1);
	});
});

describe('the tab menu', () => {
	const openTabMenu = async (index: number) => {
		fireEvent.contextMenu(tabs()[index]!);
		return screen.findByRole('menu', { name: 'Tab actions' });
	};

	it('offers Split With for a tab that is alone, listing the tabs that are free', async () => {
		const h = await renderWorkspace();
		await h.tabs.openTab(DOCS);
		await h.tabs.openTab(MUSIC);
		await h.tabs.joinPair([2, 3], 'sideBySide');
		await h.tabs.openTab(MUSIC);
		await waitFor(() => expect(tabs()).toHaveLength(4));
		await openTabMenu(0);
		fireEvent.click(await item('Split With'));
		// Tabs 2 and 3 are already a pair, so only the fourth is offered.
		expect(await screen.findAllByRole('menuitem', { name: 'music' })).toHaveLength(1);
		expect(screen.queryByRole('menuitem', { name: 'docs' })).toBeNull();
		fireEvent.click(screen.getByRole('menuitem', { name: 'music' }));
		await waitFor(async () =>
			expect((await snapshot(h)).pairs.map((pair) => pair.panes)).toContainEqual([1, 4]),
		);
	});

	it('disables Split With when no other tab is free', async () => {
		await renderWorkspace();
		await openTabMenu(0);
		expect(await item('Split With')).toHaveAttribute('aria-disabled', 'true');
	});

	it('offers the pair actions on either half, with Separate Tabs', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await openTabMenu(1);
		expect(screen.queryByRole('menuitem', { name: 'Split With' })).toBeNull();
		expect(await item('Swap Panes')).toBeInTheDocument();
		expect(await item('Layout')).toBeInTheDocument();
		expect(await item('Reset Sizes')).toBeInTheDocument();
		expect(await item('Close Both')).toBeInTheDocument();
		fireEvent.click(await item('Separate Tabs'));
		await waitFor(async () => expect((await snapshot(h)).pairs).toEqual([]));
	});

	it('colours both halves when one is coloured', async () => {
		const h = await renderWorkspace();
		await joined(h);
		await openTabMenu(1);
		fireEvent.click(await item('Colour'));
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: 'Blue' }));
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.map((tab) => tab.colour)).toEqual(['blue', 'blue']),
		);
	});
});

describe('F3', () => {
	it('splits a single tab onto the same folder and focuses the new pane', async () => {
		const h = await renderWorkspace();
		await split();
		const state = await snapshot(h);
		expect(state.tabs).toHaveLength(2);
		expect(state.tabs[1]!.location.uri).toBe(state.tabs[0]!.location.uri);
		expect(state.pairs[0]!.origin).toEqual({ kind: 'toggle', created: 2 });
		expect(state.active).toBe(2);
		await waitFor(() => expect(panes()[1]).toHaveAttribute('data-active'));
		await waitFor(() =>
			expect(document.activeElement).toBe(within(panes()[1]!).getByRole('listbox')),
		);
		await waitFor(() => expect(live()).toHaveTextContent('Split test into two panes'));
	});

	it('closes the pane it created, with an Undo toast, and keeps it for Reopen Closed Tab', async () => {
		const h = await renderWorkspace();
		await split();
		// The new pane goes somewhere, so there is history to keep.
		await h.tabs.navigate(2, DOCS);
		await waitFor(() => expect(within(panes()[1]!).queryByText('report.pdf')).not.toBeNull());
		fireEvent.keyDown(window, { key: 'F3' });
		await waitFor(() => expect(panes()).toHaveLength(0));
		const state = await snapshot(h);
		expect(state.tabs.map((tab) => tab.id)).toEqual([1]);
		expect(state.active).toBe(1);
		expect(state.closed[0]!.tab.id).toBe(2);
		expect(state.closed[0]!.tab.back).toHaveLength(1);

		const toast = await screen.findByText('Closed the split pane docs');
		expect(toast.closest('[role="status"]')).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Undo' })).toBeInTheDocument();
	});

	it('brings the pane back, history and all, with Undo', async () => {
		const h = await renderWorkspace();
		await split();
		await h.tabs.navigate(2, DOCS);
		await act(async () => {
			await h.tabs.setPairSizes(1, [300, 700]);
		});
		fireEvent.keyDown(window, { key: 'F3' });
		fireEvent.click(await screen.findByRole('button', { name: 'Undo' }));
		await waitFor(() => expect(panes()).toHaveLength(2));
		const state = await snapshot(h);
		expect(state.tabs.map((tab) => tab.id)).toEqual([1, 2]);
		expect(state.tabs[1]!.location.display).toContain('docs');
		expect(state.tabs[1]!.back).toHaveLength(1);
		expect(state.pairs[0]!.sizes).toEqual([300, 700]);
		expect(state.closed).toEqual([]);
		await waitFor(() => expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull());
		await waitFor(() => expect(live()).toHaveTextContent('Restored the split pane docs'));
	});

	it('also offers the closed pane in Reopen Closed Tab (Ctrl+Shift+T)', async () => {
		const h = await renderWorkspace();
		await split();
		fireEvent.keyDown(window, { key: 'F3' });
		await waitFor(() => expect(panes()).toHaveLength(0));
		fireEvent.keyDown(window, { key: 'T', ctrlKey: true, shiftKey: true });
		await waitFor(async () => expect((await snapshot(h)).tabs).toHaveLength(2));
	});

	it('closes the created pane whichever pane is focused', async () => {
		const h = await renderWorkspace();
		await split();
		fireEvent.keyDown(window, { key: 'F6' });
		await waitFor(async () => expect((await snapshot(h)).active).toBe(1));
		fireEvent.keyDown(window, { key: 'F3' });
		await waitFor(async () => expect((await snapshot(h)).tabs.map((tab) => tab.id)).toEqual([1]));
	});

	it('only separates a pair that was joined by hand, without a toast', async () => {
		const h = await renderWorkspace();
		await joined(h);
		fireEvent.keyDown(window, { key: 'F3' });
		await waitFor(async () => expect((await snapshot(h)).pairs).toEqual([]));
		expect((await snapshot(h)).tabs).toHaveLength(2);
		expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull();
		await waitFor(() => expect(live()).toHaveTextContent('Separated test and docs'));
	});

	it('dismisses the toast with Escape, its button, or after a while', async () => {
		await renderWorkspace();
		await split();
		fireEvent.keyDown(window, { key: 'F3' });
		const undo = await screen.findByRole('button', { name: 'Undo' });
		fireEvent.keyDown(undo, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull());

		await split();
		fireEvent.keyDown(window, { key: 'F3' });
		fireEvent.click(await screen.findByRole('button', { name: 'Dismiss' }));
		await waitFor(() => expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull());

		vi.useFakeTimers({ shouldAdvanceTime: true });
		await split();
		fireEvent.keyDown(window, { key: 'F3' });
		await screen.findByRole('button', { name: 'Undo' });
		act(() => {
			vi.advanceTimersByTime(NOTICE_TOAST_MS + 100);
		});
		await waitFor(() => expect(screen.queryByRole('button', { name: 'Undo' })).toBeNull());
	});
});

describe('closing a half', () => {
	it('from the pane header separates, leaving the other as a single tab', async () => {
		const h = await renderWorkspace();
		await joined(h);
		fireEvent.click(screen.getByRole('button', { name: 'Close pane docs' }));
		await waitFor(() => expect(panes()).toHaveLength(0));
		const state = await snapshot(h);
		expect(state.tabs.map((tab) => tab.id)).toEqual([1]);
		expect(state.pairs).toEqual([]);
		await waitFor(() => expect(live()).toHaveTextContent('Closed docs; test is now a single tab'));
		expect(screen.queryByRole('button', { name: /^Close pane/ })).toBeNull();
	});

	it('from the tab strip does the same', async () => {
		const h = await renderWorkspace();
		await joined(h);
		fireEvent.click(screen.getByRole('button', { name: 'Close test' }));
		await waitFor(async () => expect((await snapshot(h)).tabs.map((tab) => tab.id)).toEqual([2]));
		expect((await snapshot(h)).pairs).toEqual([]);
		await waitFor(() => expect(panes()).toHaveLength(0));
	});
});

describe('the pane header', () => {
	it('shows the folder, a grip, a close button and the active state', async () => {
		const h = await renderWorkspace();
		await joined(h);
		const second = panes()[1]!;
		expect(within(second).getByText('docs')).toBeInTheDocument();
		expect(second.querySelector('[data-pane-grip]')).not.toBeNull();
		expect(within(second).getByRole('button', { name: 'Close pane docs' })).toBeInTheDocument();
		expect(within(second).queryByText('Active')).toBeNull();
	});
});
