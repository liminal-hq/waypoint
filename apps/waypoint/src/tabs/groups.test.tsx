// Verifies the group chip, the group menu, the tab menu's group items and the strip around groups
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { stubLayout } from '../test/browseHarness';
import { DOCS, HOME, MUSIC, renderWorkspace } from '../test/workspaceHarness';
import { endRename } from './groupActions';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	endRename();
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

type Harness = Awaited<ReturnType<typeof renderWorkspace>>;

const tabs = () => screen.queryAllByRole('tab');
const names = () => tabs().map((tab) => tab.getAttribute('aria-label') ?? tab.textContent);
const snapshot = (h: Harness) => h.tabs.getSnapshot();
const live = () => screen.getAllByRole('status').find((el) => /srOnly/.test(el.className))!;
const chip = (name: string | RegExp) => screen.getByRole('button', { name });
const item = (name: string | RegExp) => screen.findByRole('menuitem', { name });

/** Tabs test(1), docs(2), music(3); tabs 1 and 2 are grouped as "Group 1". */
async function withGroup(h: Harness) {
	await h.tabs.openTab(DOCS);
	await h.tabs.openTab(MUSIC);
	await h.tabs.createGroup([1, 2]);
	await waitFor(() =>
		expect(screen.getByRole('button', { name: /^Group 1,/ })).toBeInTheDocument(),
	);
}

async function openGroupMenu(name = /^Group 1,/) {
	fireEvent.contextMenu(chip(name));
	return screen.findByRole('menu', { name: 'Group actions' });
}

describe('the group chip', () => {
	it('is a button before its tabs, not a tab, and names the group and its size', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		const button = chip('Group 1, tab group, 2 tabs');
		expect(button).toHaveAttribute('aria-expanded', 'true');
		expect(button).toHaveTextContent('Group 1');
		expect(screen.getAllByRole('tab')).toHaveLength(3);
		expect(button.compareDocumentPosition(tabs()[0]!)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
		// Its tabs say which group they are in.
		expect(tabs()[0]).toHaveAccessibleDescription('Group: Group 1');
		expect(tabs()[2]).not.toHaveAttribute('aria-describedby');
	});

	it('collapses on click and reads "Group 1 · 2", then expands again', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		fireEvent.click(chip(/^Group 1,/));
		await waitFor(() => expect(tabs()).toHaveLength(1));
		expect(chip(/^Group 1,/)).toHaveAttribute('aria-expanded', 'false');
		expect(chip(/^Group 1,/)).toHaveTextContent('Group 1· 2');
		expect(live()).toHaveTextContent('Collapsed Group 1, 2 tabs');
		fireEvent.click(chip(/^Group 1,/));
		await waitFor(() => expect(tabs()).toHaveLength(3));
		expect(live()).toHaveTextContent('Expanded Group 1, 2 tabs');
	});

	it('toggles from the keyboard with Enter and Space', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		act(() => chip(/^Group 1,/).focus());
		fireEvent.keyDown(chip(/^Group 1,/), { key: 'Enter' });
		await waitFor(() => expect(tabs()).toHaveLength(1));
		fireEvent.keyDown(chip(/^Group 1,/), { key: ' ' });
		await waitFor(() => expect(tabs()).toHaveLength(3));
	});

	it('renames in place on double-click: Enter commits', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		fireEvent.doubleClick(chip(/^Group 1,/));
		const input = await screen.findByRole('textbox', { name: 'Group name' });
		expect(input).toHaveFocus();
		fireEvent.change(input, { target: { value: 'Site' } });
		fireEvent.keyDown(input, { key: 'Enter' });
		await waitFor(() => expect(chip('Site, tab group, 2 tabs')).toBeInTheDocument());
		expect((await snapshot(h)).groups[0]!.name).toBe('Site');
		expect(live()).toHaveTextContent('Renamed the group to Site');
		expect(chip(/^Site,/)).toHaveFocus();
	});

	it('does not toggle the group on a real double-click, and still renames', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		const button = chip(/^Group 1,/);
		fireEvent.click(button, { detail: 1 });
		await waitFor(() => expect(tabs()).toHaveLength(1));
		fireEvent.click(button, { detail: 2 });
		fireEvent.doubleClick(button);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		expect(await screen.findByRole('textbox', { name: 'Group name' })).toBeInTheDocument();
	});

	it('renames with F2; Escape cancels and an empty name keeps the old one', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		act(() => chip(/^Group 1,/).focus());
		fireEvent.keyDown(chip(/^Group 1,/), { key: 'F2' });
		let input = await screen.findByRole('textbox', { name: 'Group name' });
		fireEvent.change(input, { target: { value: 'Nope' } });
		fireEvent.keyDown(input, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
		expect((await snapshot(h)).groups[0]!.name).toBe('Group 1');

		fireEvent.keyDown(chip(/^Group 1,/), { key: 'F2' });
		input = await screen.findByRole('textbox', { name: 'Group name' });
		fireEvent.change(input, { target: { value: '   ' } });
		fireEvent.keyDown(input, { key: 'Enter' });
		await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
		expect((await snapshot(h)).groups[0]!.name).toBe('Group 1');
	});

	it('is reached by the arrow keys between tabs, which skip a collapsed group', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		// Strip order: chip, test, docs, music.
		act(() => tabs()[2]!.focus());
		fireEvent.keyDown(tabs()[2]!, { key: 'ArrowRight' });
		expect(tabs()[2]).toHaveFocus();
		fireEvent.keyDown(tabs()[2]!, { key: 'Home' });
		expect(chip(/^Group 1,/)).toHaveFocus();
		fireEvent.keyDown(chip(/^Group 1,/), { key: 'ArrowRight' });
		expect(tabs()[0]).toHaveFocus();
		fireEvent.keyDown(tabs()[0]!, { key: 'ArrowLeft' });
		expect(chip(/^Group 1,/)).toHaveFocus();

		fireEvent.keyDown(chip(/^Group 1,/), { key: 'Enter' });
		await waitFor(() => expect(tabs()).toHaveLength(1));
		fireEvent.keyDown(chip(/^Group 1,/), { key: 'ArrowRight' });
		expect(tabs()[0]).toHaveFocus();
	});

	it('is the strip tab stop when the active tab is hidden inside it', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await h.tabs.activateTab(1);
		await waitFor(() => expect(tabs()[0]).toHaveAttribute('aria-selected', 'true'));
		fireEvent.click(chip(/^Group 1,/));
		await waitFor(() => expect(tabs()).toHaveLength(1));
		expect(chip(/^Group 1,/)).toHaveAttribute('tabindex', '0');
		expect(chip(/^Group 1,/)).toHaveAccessibleName(/contains the active tab/);
		expect(tabs()[0]).toHaveAttribute('tabindex', '-1');
	});

	it('expands when one of its hidden tabs becomes active', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await h.tabs.setGroupCollapsed(1, true);
		await waitFor(() => expect(tabs()).toHaveLength(1));
		await h.tabs.activateTab(2);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		expect(live()).toHaveTextContent('Expanded Group 1 to show docs');
	});

	it('moves the whole group with Ctrl+Shift+Arrow', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		act(() => chip(/^Group 1,/).focus());
		fireEvent.keyDown(chip(/^Group 1,/), { key: 'ArrowLeft', ctrlKey: true, shiftKey: true });
		expect((await snapshot(h)).tabs.map((tab) => tab.id)).toEqual([1, 2, 3]);
		fireEvent.keyDown(chip(/^Group 1,/), { key: 'ArrowRight', ctrlKey: true, shiftKey: true });
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.map((tab) => tab.id)).toEqual([3, 1, 2]),
		);
		expect(live()).toHaveTextContent('Moved group Group 1 to position 2 of 3');
	});
});

describe('the soft limit', () => {
	async function bigGroup(h: Harness, size: number) {
		for (let i = 0; i < size - 1; i++) await h.tabs.openTab(HOME);
		await h.tabs.createGroup(Array.from({ length: size }, (_, i) => i + 1));
	}

	it('shows the count and an amber outline past eight, and a warning when a tab joins', async () => {
		const h = await renderWorkspace();
		await bigGroup(h, 8);
		await h.tabs.openTab(HOME);
		await waitFor(() => expect(chip(/^Group 1,/)).toBeInTheDocument());
		expect(chip(/^Group 1,/).parentElement).not.toHaveAttribute('data-over-limit');
		expect(chip(/^Group 1,/)).not.toHaveTextContent('· ');
		expect(screen.queryByRole('note')).toBeNull();

		await h.tabs.addToGroup(9, 1);
		await waitFor(() => expect(chip(/^Group 1,/).parentElement).toHaveAttribute('data-over-limit'));
		expect(chip(/^Group 1,/)).toHaveTextContent('· 9');
		expect(chip(/^Group 1,/)).toHaveAccessibleName(/more than 8 tabs/);
		expect(screen.getByRole('note')).toHaveTextContent('Group 1 has 9 tabs');
		expect(live()).toHaveTextContent('Group 1 has 9 tabs');
		// Nothing was refused.
		expect((await snapshot(h)).tabs.filter((tab) => tab.group === 1)).toHaveLength(9);
	});
});

describe('the group menu', () => {
	it('lists SPEC section 5.2 in order, with Save as Workspace disabled and explained', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		const menu = await openGroupMenu();
		const labels = [...menu.querySelectorAll('[role^="menuitem"]')].map((el) => el.textContent);
		expect(labels).toEqual([
			'Rename GroupF2',
			'Change Colour',
			'Collapse Group',
			'Collapse All Other Groups',
			'New Tab in Group',
			'Pin Group',
			'Sort Tabs in Group',
			'Duplicate Group',
			'Save Group as Workspace',
			'Move Group to New Window',
			'Ungroup',
			'Close Group',
		]);
		const save = await item('Save Group as Workspace');
		expect(save).toHaveAttribute('aria-disabled', 'true');
		expect(save).toHaveAttribute('title', 'Arrives with workspaces');
		expect(await item('Collapse All Other Groups')).toHaveAttribute('aria-disabled', 'true');
	});

	it('opens from the keyboard with the Menu key and Shift+F10 and returns focus to the chip', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		act(() => chip(/^Group 1,/).focus());
		fireEvent.keyDown(chip(/^Group 1,/), { key: 'ContextMenu' });
		await screen.findByRole('menu', { name: 'Group actions' });
		fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		expect(chip(/^Group 1,/)).toHaveFocus();
		fireEvent.keyDown(chip(/^Group 1,/), { key: 'F10', shiftKey: true });
		await screen.findByRole('menu', { name: 'Group actions' });
	});

	it('renames, with the name field taking the focus after the menu closes', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await openGroupMenu();
		fireEvent.click(await item(/^Rename Group/));
		const input = await screen.findByRole('textbox', { name: 'Group name' });
		await waitFor(() => expect(input).toHaveFocus());
	});

	it('sets and clears the colour, which the chip and the tabs carry', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await openGroupMenu();
		fireEvent.click(await item('Change Colour'));
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: 'Teal' }));
		await waitFor(async () => expect((await snapshot(h)).groups[0]!.colour).toBe('teal'));
		expect(chip(/^Group 1,/).parentElement).toHaveAttribute('data-colour', 'teal');
		expect(live()).toHaveTextContent('Colour of group Group 1 set to Teal');
		expect(tabs()[0]!.parentElement!.style.getPropertyValue('--wp-group-accent')).toBe(
			'var(--wp-tab-colour-teal)',
		);

		await openGroupMenu();
		fireEvent.click(await item('Change Colour'));
		expect(await screen.findByRole('menuitemcheckbox', { name: 'Teal' })).toHaveAttribute(
			'aria-checked',
			'true',
		);
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: 'None' }));
		await waitFor(async () => expect((await snapshot(h)).groups[0]!.colour).toBeNull());
	});

	it('collapses and expands, and collapses the other groups', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await h.tabs.createGroup([3]);
		await waitFor(() =>
			expect(screen.getByRole('button', { name: /^Group 2,/ })).toBeInTheDocument(),
		);
		await openGroupMenu();
		fireEvent.click(await item('Collapse All Other Groups'));
		await waitFor(async () =>
			expect((await snapshot(h)).groups.map((group) => group.collapsed)).toEqual([false, true]),
		);
		await openGroupMenu();
		fireEvent.click(await item('Collapse Group'));
		await waitFor(async () => expect((await snapshot(h)).groups[0]!.collapsed).toBe(true));
		await openGroupMenu();
		expect(await item('Expand Group')).toBeInTheDocument();
	});

	it('opens a new tab in the group, pins and unpins the group, and sorts it', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await openGroupMenu();
		fireEvent.click(await item('New Tab in Group'));
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.map((tab) => tab.group)).toEqual([1, 1, 1, null]),
		);

		await openGroupMenu();
		fireEvent.click(await item('Pin Group'));
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.filter((tab) => tab.pinned).map((tab) => tab.group)).toEqual([
				1, 1, 1,
			]),
		);
		expect(live()).toHaveTextContent('Pinned group Group 1');
		await openGroupMenu();
		expect(await item('Unpin Group')).toBeInTheDocument();
		fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
		await h.tabs.pinTab(1, false);

		await waitFor(() => expect(tabs()[0]).toHaveTextContent('test'));
		await openGroupMenu();
		fireEvent.click(await item('Sort Tabs in Group'));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'By Name' }));
		await waitFor(async () => {
			const order = (await snapshot(h)).tabs
				.filter((tab) => tab.group === 1)
				.map((tab) => tab.location.display.split('/').pop());
			expect(order).toEqual(['docs', 'docs', 'test']);
		});
		expect(live()).toHaveTextContent('Sorted Group 1 By Name');
	});

	it('duplicates, ungroups and closes', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await openGroupMenu();
		fireEvent.click(await item('Duplicate Group'));
		await waitFor(async () => expect((await snapshot(h)).groups).toHaveLength(2));

		// The copy keeps the name and sits after the original.
		fireEvent.contextMenu(screen.getAllByRole('button', { name: /^Group 1,/ })[1]!);
		await screen.findByRole('menu', { name: 'Group actions' });
		fireEvent.click(await item('Close Group'));
		await waitFor(async () => expect((await snapshot(h)).groups).toHaveLength(1));
		expect(live()).toHaveTextContent('Closed group Group 1');

		await openGroupMenu();
		fireEvent.click(await item('Ungroup'));
		await waitFor(async () => expect((await snapshot(h)).groups).toHaveLength(0));
		expect(screen.queryByRole('button', { name: /tab group/ })).toBeNull();
		expect(live()).toHaveTextContent('Ungrouped Group 1');
	});

	it('closing every tab of the window with Close Group leaves a tab at Home', async () => {
		const h = await renderWorkspace();
		await h.tabs.createGroup([1]);
		await waitFor(() => expect(chip(/^Group 1,/)).toBeInTheDocument());
		await openGroupMenu();
		fireEvent.click(await item('Close Group'));
		await waitFor(async () => {
			const current = await snapshot(h);
			expect(current.tabs).toHaveLength(1);
			expect(current.tabs[0]!.group).toBeNull();
		});
	});

	it('moves the group to a new window when the factory makes one', async () => {
		const made: string[] = [];
		const store = new FakeTabsStore({ createWindow: (label) => void made.push(label) });
		const h = await renderWorkspace(undefined, new FakeTabsApi(store, 'main-1'));
		await withGroup(h);
		await openGroupMenu();
		fireEvent.click(await item('Move Group to New Window'));
		await waitFor(() => expect(made).toHaveLength(1));
		await waitFor(() => expect(screen.queryByRole('button', { name: /tab group/ })).toBeNull());
		expect(live()).toHaveTextContent('Moved group Group 1 to a new window');
	});

	it('says so, without failing, when windows cannot be made yet', async () => {
		const store = new FakeTabsStore({
			createWindow: () => {
				throw 'creating windows is not available yet';
			},
		});
		const h = await renderWorkspace(undefined, new FakeTabsApi(store, 'main-1'));
		await withGroup(h);
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		await openGroupMenu();
		fireEvent.click(await item('Move Group to New Window'));
		await waitFor(() =>
			expect(live()).toHaveTextContent('Moving Group 1 to a new window is not available yet'),
		);
		expect(warn).not.toHaveBeenCalled();
		expect(chip(/^Group 1,/)).toBeInTheDocument();
		expect((await snapshot(h)).groups).toHaveLength(1);
	});
});

describe('the tab menu and groups', () => {
	const openTabMenu = async (index: number) => {
		fireEvent.contextMenu(tabs()[index]!);
		return screen.findByRole('menu', { name: 'Tab actions' });
	};

	it('starts a new group with the name open for editing', async () => {
		const h = await renderWorkspace();
		await h.tabs.openTab(DOCS);
		await waitFor(() => expect(tabs()).toHaveLength(2));
		await openTabMenu(1);
		fireEvent.click(await item('Add to Group'));
		expect(screen.queryAllByRole('menuitem', { name: /^Group/ })).toHaveLength(0);
		fireEvent.click(await item('New Group'));
		const input = await screen.findByRole('textbox', { name: 'Group name' });
		expect(input).toHaveValue('Group 1');
		await waitFor(() => expect(input).toHaveFocus());
		expect(live()).toHaveTextContent('Created group Group 1');
		expect((await snapshot(h)).tabs.map((tab) => tab.group)).toEqual([null, 1]);
	});

	it('adds a tab to an existing group and says how many it now holds', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await openTabMenu(2);
		fireEvent.click(await item('Add to Group'));
		fireEvent.click(await item('Group 1'));
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.map((tab) => tab.group)).toEqual([1, 1, 1]),
		);
		expect(live()).toHaveTextContent('Added music to Group 1, now 3 tabs');
	});

	it('removes a tab from its group, and offers the other groups but not its own', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await h.tabs.createGroup([3]);
		await waitFor(() => expect(chip(/^Group 2,/)).toBeInTheDocument());
		await openTabMenu(0);
		fireEvent.click(await item('Add to Group'));
		expect(await item('Group 2')).toBeInTheDocument();
		expect(screen.queryByRole('menuitem', { name: 'Group 1' })).toBeNull();
		fireEvent.keyDown(screen.getAllByRole('menu').at(-1)!, { key: 'Escape' });
		fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());

		await openTabMenu(0);
		fireEvent.click(await item('Remove from Group'));
		await waitFor(async () => expect((await snapshot(h)).tabs[0]!.group).toBeNull());
		expect(live()).toHaveTextContent('Removed test from Group 1');
		await openTabMenu(0);
		expect(screen.queryByRole('menuitem', { name: 'Remove from Group' })).toBeNull();
	});

	it('names the group in the pin item, since pinning a grouped tab pins the group', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await openTabMenu(0);
		expect(await item('Pin Group “Group 1”')).toBeInTheDocument();
		expect(screen.queryByRole('menuitem', { name: 'Pin Tab' })).toBeNull();
	});
});

describe('reordering around groups', () => {
	const stubSpans = () =>
		vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
			this: HTMLElement,
		) {
			const index = this.getAttribute('data-index');
			const left = index === null ? 0 : Number(index) * 100;
			return { left, right: left + 100, top: 0, bottom: 30, width: 100, height: 30 } as DOMRect;
		});

	it('keeps a dragged group member inside its group', async () => {
		stubSpans();
		const h = await renderWorkspace();
		await withGroup(h);
		const slot = tabs()[0]!.parentElement!;
		fireEvent.pointerDown(slot, { button: 0, clientX: 50, pointerId: 1 });
		fireEvent.pointerMove(slot, { clientX: 290, pointerId: 1 });
		fireEvent.pointerUp(slot, { clientX: 290, pointerId: 1 });
		// Dragged past the group's own end it takes the last place in the group.
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.map((tab) => tab.id)).toEqual([2, 1, 3]),
		);
		expect((await snapshot(h)).tabs[1]!.group).toBe(1);
	});

	it('drops an ungrouped tab beside a group, never inside it', async () => {
		stubSpans();
		const h = await renderWorkspace();
		await withGroup(h);
		// Strip: test(0) docs(1) | music(2). Drag music to the middle of the group.
		const slot = tabs()[2]!.parentElement!;
		fireEvent.pointerDown(slot, { button: 0, clientX: 250, pointerId: 1 });
		fireEvent.pointerMove(slot, { clientX: 100, pointerId: 1 });
		fireEvent.pointerUp(slot, { clientX: 100, pointerId: 1 });
		await waitFor(() => expect(slot).not.toHaveAttribute('data-dragging'));
		const after = await snapshot(h);
		expect(after.tabs.map((tab) => tab.id)).toEqual([1, 2, 3]);
		expect(after.tabs[2]!.group).toBeNull();
	});

	it('steps an ungrouped tab over a whole group with the keyboard', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		act(() => tabs()[2]!.focus());
		fireEvent.keyDown(tabs()[2]!, { key: 'ArrowLeft', ctrlKey: true, shiftKey: true });
		await waitFor(async () =>
			expect((await snapshot(h)).tabs.map((tab) => tab.id)).toEqual([3, 1, 2]),
		);
		expect((await snapshot(h)).tabs.map((tab) => tab.group)).toEqual([null, 1, 1]);
	});
});

describe('the layout', () => {
	it('lists pinned tabs first, then each group with its chip, and tab names are unchanged', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await h.tabs.pinTab(3, true);
		await waitFor(() => expect(names()).toEqual(['music', 'test', 'docs']));
	});

	it('shows a pinned group as a compact chip among the pinned tabs', async () => {
		const h = await renderWorkspace();
		await withGroup(h);
		await h.tabs.pinTab(1, true);
		await waitFor(() => expect(chip(/^Group 1,/).parentElement).toHaveAttribute('data-pinned'));
		expect(chip(/^Group 1,/)).not.toHaveTextContent('Group 1');
		expect(chip(/^Group 1,/)).toHaveAttribute('title', 'Group 1');
	});
});
