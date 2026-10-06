// Verifies the Shelf in the whole browsing area: Ctrl+B, Add to Shelf, drops onto it, drags out of it, the keyboard and a second window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dismissNotice } from '../app/notices';
import { PLAN_REST_MS } from '../dnd/fileDrag';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { fileLocation, makeEntry, type FakeVfsClient } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, DOCS, HOME, renderWorkspace } from '../test/workspaceHarness';

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

async function mount(options: { store?: FakeTabsStore; label?: string } = {}) {
	const vfs = createTree();
	const store = options.store ?? FakeTabsStore.singleWindow();
	const tabs = new FakeTabsApi(store, options.label ?? 'main-1');
	await tabs.openTab(HOME);
	const ops = createFakeOpsClient({
		resolveSelection: async (handle, spec) =>
			Promise.all(spec.ids.map((id) => vfs.entryLocation(handle, id))),
	});
	const view = await renderWorkspace(vfs, tabs, undefined, { ops });
	return { ops, tabs, vfs, store, view };
}

const shelf = () => screen.queryByRole('complementary', { name: 'Shelf' });
const openShelf = async () => {
	fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
	return await screen.findByRole('complementary', { name: 'Shelf' });
};
const options = () =>
	within(screen.getAllByRole('listbox', { name: /^Files/ })[0]!).getAllByRole('option');
const row = (name: string) => options().find((option) => option.textContent?.includes(name))!;
const levelled = (level: number) => {
	const tree = screen.queryByRole('tree', { name: 'Shelf items' });
	return tree
		? [...tree.querySelectorAll<HTMLElement>(`[role="treeitem"][aria-level="${level}"]`)]
		: [];
};
const shelved = () => levelled(2);
const groupRows = () => levelled(1);
const submits = (ops: FakeOpsClient) => ops.calls.filter((call) => call[0] === 'submit');
const lastSubmit = (ops: FakeOpsClient) => submits(ops).at(-1)![1] as Record<string, unknown>;
const pointAt = (...elements: Element[]) => {
	document.elementsFromPoint = () => elements;
};
const moveTo = (x: number, keys: { ctrlKey?: boolean } = {}) =>
	fireEvent.pointerMove(window, { pointerId: 1, clientX: x, clientY: 100, ...keys });
const release = (x: number) =>
	fireEvent.pointerUp(window, { pointerId: 1, clientX: x, clientY: 100 });
const rest = () =>
	act(async () => void (await new Promise((done) => setTimeout(done, PLAN_REST_MS + 60))));
const pill = () => document.querySelector('[data-drag-pill]')?.textContent ?? null;

async function grab(element: Element) {
	fireEvent.pointerDown(element, { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
	fireEvent.pointerMove(window, { pointerId: 1, clientX: 120, clientY: 100 });
}

/** A click as a pointer makes it: press, release, click. */
async function clickRow(element: Element, keys: { ctrlKey?: boolean; shiftKey?: boolean } = {}) {
	fireEvent.pointerDown(element, { pointerId: 1, button: 0, clientX: 100, clientY: 100, ...keys });
	fireEvent.pointerUp(window, { pointerId: 1, clientX: 100, clientY: 100 });
	fireEvent.click(element, keys);
}

/** Puts `names` of the home folder on the Shelf through the session, as another window or an earlier run would. */
async function shelve(tabs: FakeTabsApi, ...paths: string[]) {
	await tabs.addToShelf(paths.map((p) => fileLocation(p)));
}

describe('showing and hiding', () => {
	it('is hidden at first, and Ctrl+B shows and hides it, with the button saying which', async () => {
		await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		expect(shelf()).toBeNull();
		const toggle = within(screen.getByRole('group', { name: 'Status bar' })).getByRole('button', {
			name: 'Shelf',
		});
		expect(toggle).toHaveAttribute('aria-pressed', 'false');
		const panel = await openShelf();
		expect(toggle).toHaveAttribute('aria-pressed', 'true');
		expect(within(panel).getByText('The Shelf is empty')).toBeInTheDocument();
		expect(within(panel).getByText(/Drag files here, or choose Add to Shelf/)).toBeInTheDocument();
		fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
		expect(shelf()).toBeNull();
		fireEvent.click(toggle);
		expect(shelf()).not.toBeNull();
		fireEvent.click(within(shelf()!).getByRole('button', { name: 'Hide the Shelf' }));
		expect(shelf()).toBeNull();
	});

	it('docks under the panes, beside the sidebar, and above the status bar', async () => {
		await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		const panel = await openShelf();
		const files = document.getElementById('wp-tabpanel')!;
		const column = files.parentElement!;
		// The dock shares the panes' column, after them, so the sidebar is not under it.
		expect(panel.parentElement).toBe(column);
		expect(files.compareDocumentPosition(panel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
		// The column is one child of the row it shares with the sidebar, which is not inside it.
		const row = column.parentElement!;
		expect(row.lastElementChild).toBe(column);
		expect(column.querySelector('nav')).toBeNull();
		// The status bar is below everything, outside the row of sidebar and column.
		const toggle = within(screen.getByRole('group', { name: 'Status bar' })).getByRole('button', {
			name: 'Shelf',
		});
		expect(row.contains(toggle)).toBe(false);
	});

	it('leaves Ctrl+B to a text field', async () => {
		await mount();
		const input = document.createElement('input');
		document.body.append(input);
		fireEvent.keyDown(input, { key: 'b', ctrlKey: true });
		expect(shelf()).toBeNull();
		input.remove();
	});
});

describe('Add to Shelf', () => {
	it('puts the right-clicked entry on the Shelf from its menu', async () => {
		const { tabs } = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await openShelf();
		fireEvent.contextMenu(row('notes.txt'));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Add to Shelf' }));
		await waitFor(() => expect(shelved()).toHaveLength(1));
		expect(shelved()[0]).toHaveTextContent('notes.txt');
		expect((await tabs.getSnapshot()).shelf.map((i) => i.location.display)).toEqual([
			'/home/test/notes.txt',
		]);
		// The group is the folder the item came from, with its count.
		expect(groupRows()[0]!).toHaveTextContent('test (1)');
	});

	it('puts a whole selection on it', async () => {
		const { tabs } = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		fireEvent.click(row('notes.txt'));
		fireEvent.click(row('photo.jpg'), { ctrlKey: true });
		fireEvent.contextMenu(row('photo.jpg'));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Add to Shelf' }));
		await waitFor(async () => expect((await tabs.getSnapshot()).shelf).toHaveLength(2));
	});
});

describe('dropping onto the Shelf', () => {
	it('adds the dragged items as references: no job, and the pill says so', async () => {
		const { ops, tabs } = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		const panel = await openShelf();
		fireEvent.click(row('notes.txt'));
		fireEvent.click(row('photo.jpg'), { ctrlKey: true });
		await grab(row('photo.jpg'));
		pointAt(panel);
		moveTo(140);
		await waitFor(() => expect(pill()).toContain('Add 2 items to the Shelf'));
		expect(panel).toHaveAttribute('data-drop-over', 'ok');
		release(140);
		await waitFor(() => expect(shelved()).toHaveLength(2));
		expect(submits(ops)).toHaveLength(0);
		expect((await tabs.getSnapshot()).shelf.map((i) => i.name)).toEqual(['notes.txt', 'photo.jpg']);
		expect(panel).not.toHaveAttribute('data-drop-over');
	});

	it('names one item by its name', async () => {
		await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		const panel = await openShelf();
		await grab(row('notes.txt'));
		pointAt(panel);
		moveTo(140);
		await waitFor(() => expect(pill()).toContain('Add notes.txt to the Shelf'));
		fireEvent.keyDown(window, { key: 'Escape' });
	});

	it('says so when the Shelf is full, and adds nothing', async () => {
		const { tabs } = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await tabs.addToShelf(Array.from({ length: 500 }, (_, i) => fileLocation(`/x/f${i}`)));
		const panel = await openShelf();
		await grab(row('notes.txt'));
		pointAt(panel);
		moveTo(140);
		release(140);
		await screen.findByText('The Shelf is full: it holds at most 500 items');
		expect((await tabs.getSnapshot()).shelf).toHaveLength(500);
	}, 30_000);
});

describe('the Shelf’s items', () => {
	it('lists them newest first under their folder, and marks one whose file is gone', async () => {
		const { tabs } = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await shelve(tabs, '/home/test/notes.txt', '/home/test/gone.txt', '/home/test/docs/report.pdf');
		await openShelf();
		await waitFor(() => expect(shelved()).toHaveLength(3));
		expect(groupRows().map((g) => g.textContent)).toEqual(['docs (1)', 'test (2)']);
		expect(shelved().map((i) => i.querySelector('[class*=name]')?.textContent)).toEqual([
			'report.pdf',
			'gone.txt',
			'notes.txt',
		]);
		await waitFor(() => expect(within(shelved()[1]!).getByText('Missing')).toBeInTheDocument());
		expect(within(shelved()[0]!).queryByText('Missing')).toBeNull();
		expect(shelved()[1]).toHaveAttribute('data-missing');
	});

	it('collapses a group', async () => {
		const { tabs } = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await shelve(tabs, '/home/test/notes.txt', '/home/test/docs/report.pdf');
		await openShelf();
		await waitFor(() => expect(shelved()).toHaveLength(2));
		fireEvent.click(groupRows().find((g) => /docs \(1\)/.test(g.textContent ?? ''))!);
		expect(shelved()).toHaveLength(1);
		fireEvent.click(groupRows().find((g) => /docs \(1\)/.test(g.textContent ?? ''))!);
		expect(shelved()).toHaveLength(2);
	});

	it('removes one with its button, saying the file is untouched', async () => {
		const { tabs, vfs } = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await shelve(tabs, '/home/test/notes.txt');
		await openShelf();
		await waitFor(() => expect(shelved()).toHaveLength(1));
		const button = within(shelved()[0]!).getByRole('button', {
			name: 'Remove notes.txt from the Shelf',
		});
		expect(button).toHaveAttribute('title', 'Remove from the Shelf (the file is not deleted)');
		fireEvent.click(button);
		await waitFor(() => expect(shelved()).toHaveLength(0));
		expect(screen.getByText('The Shelf is empty')).toBeInTheDocument();
		expect(await vfs.getRange).toBeDefined();
		expect(row('notes.txt')).toBeDefined();
	});

	it('is shared with another window', async () => {
		const store = FakeTabsStore.singleWindow();
		const first = await mount({ store });
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await first.tabs.addToShelf([fileLocation('/home/test/notes.txt')]);
		await openShelf();
		await waitFor(() => expect(shelved()).toHaveLength(1));
		// A second window over the same store sees what the first put there, and what it does reaches the first.
		const second = new FakeTabsApi(store, 'main-2');
		await second.openWindow(HOME);
		await second.removeFromShelf([(await second.getSnapshot()).shelf[0]!.id]);
		await waitFor(() => expect(screen.getByText('The Shelf is empty')).toBeInTheDocument());
	});
});

describe('the keyboard', () => {
	// Rows, newest first: the docs group, report.pdf, the home folder's group, photo.jpg, notes.txt.
	async function withItems() {
		const m = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await shelve(
			m.tabs,
			'/home/test/notes.txt',
			'/home/test/photo.jpg',
			'/home/test/docs/report.pdf',
		);
		await openShelf();
		await waitFor(() => expect(shelved()).toHaveLength(3));
		fireEvent.focus(tree());
		return m;
	}
	const tree = () => screen.getByRole('tree', { name: 'Shelf items' });
	const key = (k: string, extra: Record<string, boolean> = {}) =>
		fireEvent.keyDown(tree(), { key: k, ...extra });
	const selected = () => shelved().filter((i) => i.getAttribute('aria-selected') === 'true');

	it('starts on the first row, moves along the strip and extends a selection with Shift', async () => {
		await withItems();
		expect(tree().getAttribute('aria-activedescendant')).toContain('group');
		key('ArrowRight');
		expect(tree().getAttribute('aria-activedescendant')).toContain('item');
		key(' ');
		expect(selected()).toHaveLength(1);
		expect(selected()[0]).toBe(shelved()[0]);
		key('ArrowRight'); // the home folder's group
		key('ArrowRight', { shiftKey: true }); // photo.jpg, from the anchor
		expect(selected()).toHaveLength(2);
		key('End', { shiftKey: true });
		expect(selected()).toHaveLength(3);
		key('Home');
		expect(tree().getAttribute('aria-activedescendant')).toContain('group');
	});

	it('selects all with Ctrl+A and nothing with Escape', async () => {
		await withItems();
		key('a', { ctrlKey: true });
		expect(selected()).toHaveLength(3);
		key('Escape');
		expect(selected()).toHaveLength(0);
	});

	it('collapses and opens a group with Enter, and Left and Right move along the strip', async () => {
		await withItems();
		key('Enter');
		expect(shelved()).toHaveLength(2);
		key('Enter');
		expect(shelved()).toHaveLength(3);
		key('ArrowRight'); // into the group
		expect(tree().getAttribute('aria-activedescendant')).toContain('item');
		key('ArrowLeft');
		expect(tree().getAttribute('aria-activedescendant')).toContain('group');
		key('ArrowLeft'); // the strip starts here
		expect(tree().getAttribute('aria-activedescendant')).toContain('group');
	});

	it('moves between the lines of a wrapping strip with Up and Down, and stays put on one line', async () => {
		await withItems();
		key('ArrowDown');
		key('ArrowUp');
		expect(tree().getAttribute('aria-activedescendant')).toContain('group');
		// Two lines: the first three rows, then the last two (jsdom has no layout, so say where they sit).
		const rows = [...tree().querySelectorAll<HTMLElement>('[role="treeitem"]')];
		rows.forEach((element, index) => {
			const line = index < 3 ? 0 : 1;
			const column = index < 3 ? index : index - 3;
			element.getBoundingClientRect = () =>
				({ top: line * 40, left: column * 100, width: 100, height: 36 }) as DOMRect;
		});
		key('ArrowRight');
		key('ArrowRight'); // the home folder's group, at the end of the first line
		key('ArrowDown'); // the nearest tile across on the second line
		expect(tree().getAttribute('aria-activedescendant')).toBe(rows[4]!.id);
		key('ArrowUp');
		expect(tree().getAttribute('aria-activedescendant')).toBe(rows[1]!.id);
		key('ArrowUp'); // no line above the first
		expect(tree().getAttribute('aria-activedescendant')).toBe(rows[1]!.id);
	});

	it('removes the selection with Delete, without touching a file', async () => {
		const { tabs } = await withItems();
		key('ArrowRight');
		key(' ');
		key('ArrowRight');
		key('ArrowRight', { shiftKey: true });
		key('Delete');
		await waitFor(() => expect(shelved()).toHaveLength(1));
		expect((await tabs.getSnapshot()).shelf).toHaveLength(1);
		expect(row('notes.txt')).toBeDefined();
	});

	it('opens a folder in the pane with Enter, and shows a file in its folder in a new tab', async () => {
		const { tabs } = await withItems();
		key('ArrowRight');
		key('Enter');
		await waitFor(async () => expect((await tabs.getSnapshot()).tabs).toHaveLength(2));
		const snapshot = await tabs.getSnapshot();
		expect(snapshot.tabs[1]?.location).toEqual(DOCS);
		expect(snapshot.tabs[1]?.hints.focused).toBe('report.pdf');
	});

	it('copies the selected items for a paste', async () => {
		const { ops } = await withItems();
		key('ArrowRight');
		key(' ');
		key('c', { ctrlKey: true });
		await waitFor(() => expect(ops.calls.some((call) => call[0] === 'setClipboard')).toBe(true));
	});

	it('opens the item menu with the menu key and runs Remove from it', async () => {
		const { tabs } = await withItems();
		key('ArrowRight');
		key('ContextMenu');
		const menu = await screen.findByRole('menu', { name: 'Shelf item' });
		const labels = within(menu)
			.getAllByRole('menuitem')
			.map((i) => i.textContent?.replace(/(Enter|Ctrl\+C|Delete)$/, ''));
		expect(labels).toEqual(['Open', 'Reveal in Folder', 'Copy', 'Copy Path', 'Remove from Shelf']);
		fireEvent.click(within(menu).getByRole('menuitem', { name: /Remove from Shelf/ }));
		await waitFor(async () => expect((await tabs.getSnapshot()).shelf).toHaveLength(2));
	});
});

describe('Clear Shelf', () => {
	it('asks first when there is more than one item, and clears on confirm', async () => {
		const { tabs } = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await shelve(tabs, '/home/test/notes.txt', '/home/test/photo.jpg');
		const panel = await openShelf();
		await waitFor(() => expect(shelved()).toHaveLength(2));
		fireEvent.click(within(panel).getByRole('button', { name: 'Shelf options' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: /Clear Shelf/ }));
		const dialog = await screen.findByRole('dialog');
		expect(
			within(dialog).getByText(/All 2 items will be removed from the Shelf/),
		).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
		expect((await tabs.getSnapshot()).shelf).toHaveLength(2);
		fireEvent.click(within(panel).getByRole('button', { name: 'Shelf options' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: /Clear Shelf/ }));
		fireEvent.click(
			within(await screen.findByRole('dialog')).getByRole('button', { name: 'Clear Shelf' }),
		);
		await waitFor(async () => expect((await tabs.getSnapshot()).shelf).toHaveLength(0));
	});

	it('does not ask for a single item', async () => {
		const { tabs } = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await shelve(tabs, '/home/test/notes.txt');
		const panel = await openShelf();
		await waitFor(() => expect(shelved()).toHaveLength(1));
		fireEvent.click(within(panel).getByRole('button', { name: 'Shelf options' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: /Clear Shelf/ }));
		await waitFor(async () => expect((await tabs.getSnapshot()).shelf).toHaveLength(0));
		expect(screen.queryByRole('dialog')).toBeNull();
	});
});

describe('resizing', () => {
	it('is taller by dragging its top edge up, and by the arrow keys, within limits', async () => {
		await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		const panel = await openShelf();
		const edge = within(panel).getByRole('separator', { name: 'Resize the Shelf' });
		expect(edge).toHaveAttribute('aria-valuenow', '132');
		fireEvent.pointerDown(edge, { pointerId: 2, button: 0, clientY: 500 });
		fireEvent.pointerMove(edge, { pointerId: 2, clientY: 450 });
		expect(edge).toHaveAttribute('aria-valuenow', '182');
		fireEvent.pointerUp(edge, { pointerId: 2, clientY: 450 });
		fireEvent.keyDown(edge, { key: 'ArrowDown' });
		expect(edge).toHaveAttribute('aria-valuenow', '166');
		fireEvent.keyDown(edge, { key: 'ArrowUp', shiftKey: true });
		fireEvent.keyDown(edge, { key: 'ArrowUp', shiftKey: true });
		expect(edge).toHaveAttribute('aria-valuenow', '294');
		for (let i = 0; i < 6; i++) fireEvent.keyDown(edge, { key: 'ArrowUp', shiftKey: true });
		expect(edge).toHaveAttribute('aria-valuenow', '360');
		fireEvent.doubleClick(edge);
		expect(edge).toHaveAttribute('aria-valuenow', '132');
		expect(panel.style.height).toBe('132px');
	});

	it('goes back to where it was when Escape abandons the drag', async () => {
		await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		const panel = await openShelf();
		const edge = within(panel).getByRole('separator', { name: 'Resize the Shelf' });
		fireEvent.pointerDown(edge, { pointerId: 2, button: 0, clientY: 500 });
		fireEvent.pointerMove(edge, { pointerId: 2, clientY: 400 });
		expect(edge).toHaveAttribute('aria-valuenow', '232');
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(edge).toHaveAttribute('aria-valuenow', '132');
	});
});

describe('dragging out of the Shelf', () => {
	async function shelved1(vfs?: FakeVfsClient) {
		const m = await mount();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await shelve(m.tabs, '/home/test/notes.txt', '/home/test/photo.jpg');
		await openShelf();
		await waitFor(() => expect(shelved()).toHaveLength(2));
		void vfs;
		return m;
	}

	it('moves the dragged item into a folder row by the usual rule, as a job on its location', async () => {
		const { ops } = await shelved1();
		const [photo] = shelved();
		await grab(photo!);
		expect(pill()).toContain('Dragging photo.jpg');
		pointAt(row('docs'));
		moveTo(160);
		await waitFor(() => expect(pill()).toContain('Move or copy photo.jpg to docs'));
		await rest();
		await waitFor(() => expect(pill()).toContain('Move photo.jpg to docs'));
		release(160);
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'move' },
			sources: { kind: 'locations', locations: [{ uri: 'file:///home/test/photo.jpg' }] },
			destination: { uri: 'file:///home/test/docs' },
			originWindow: 'main-1',
		});
	});

	it('drags the whole selection, and copies with Ctrl', async () => {
		const { ops } = await shelved1();
		await clickRow(shelved()[0]!);
		await clickRow(shelved()[1]!, { ctrlKey: true });
		await grab(shelved()[1]!);
		pointAt(row('music'));
		moveTo(160);
		await waitFor(() => expect(pill()).toContain('2 items to music'));
		fireEvent.pointerUp(window, { pointerId: 1, clientX: 160, clientY: 100, ctrlKey: true });
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'copy' },
			sources: { kind: 'locations', locations: [{}, {}] },
		});
	});

	it('drops the entry of an item whose file left, and keeps one whose file is still there', async () => {
		const { vfs, tabs, ops } = await shelved1();
		await clickRow(shelved()[0]!);
		await clickRow(shelved()[1]!, { ctrlKey: true });
		await grab(shelved()[1]!);
		pointAt(row('docs'));
		moveTo(160);
		await rest();
		release(160);
		await vi.waitFor(() => expect(ops.jobs()).toHaveLength(1));
		// photo.jpg left the home folder; notes.txt stayed (a conflict skipped it).
		vfs.setFolder(HOME, [
			makeEntry(1, 'docs', { kind: 'directory' }),
			makeEntry(2, 'music', { kind: 'directory' }),
			makeEntry(3, 'notes.txt'),
		]);
		const job = ops.jobs()[0]!;
		ops.start(job.id);
		ops.done(job.id);
		await waitFor(async () =>
			expect((await tabs.getSnapshot()).shelf.map((i) => i.name)).toEqual(['notes.txt']),
		);
	});

	it('keeps every entry of a move that did not finish', async () => {
		const { tabs, ops } = await shelved1();
		await grab(shelved()[0]!);
		pointAt(row('docs'));
		moveTo(160);
		await rest();
		release(160);
		await vi.waitFor(() => expect(ops.jobs()).toHaveLength(1));
		await new Promise((done) => setTimeout(done, 50));
		expect((await tabs.getSnapshot()).shelf).toHaveLength(2);
	});

	it('does not drop onto the Trash, the + button or the Shelf itself, and says why', async () => {
		await shelved1();
		const panel = shelf()!;
		await grab(shelved()[0]!);
		pointAt(panel);
		moveTo(160);
		await waitFor(() => expect(pill()).toContain('these items are already on the Shelf'));
		fireEvent.keyDown(window, { key: 'Escape' });
	});
});
