// Verifies the Trash in the Main window: the Trash place, its list and columns, and the actions on what is in it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, renderWorkspace } from '../test/workspaceHarness';
import { FakeTrashClient, fakeJobSnapshot } from './fakeTrashClient';
import { TRASH_LOCATION } from './trashLocation';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280, 900);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

const DAY = 86_400_000;
const NOW = 1_700_000_000_000;

function trashed(id: number, name: string, daysAgo: number, overrides: Partial<Entry> = {}): Entry {
	return makeEntry(id, name, {
		modifiedMs: null,
		originalPath: '/home/test/docs',
		deletedMs: NOW - daysAgo * DAY,
		...overrides,
	});
}

const ITEMS = [
	trashed(1, 'report.pdf', 1),
	trashed(2, 'old drafts', 12, { kind: 'directory', size: 5000 }),
	trashed(3, 'photo.jpg', 40, { originalPath: '/home/test/Pictures' }),
];

async function setup(info: { count?: number; available?: boolean; reason?: string | null } = {}) {
	const client = createTree();
	client.markTrash(TRASH_LOCATION);
	client.setFolder(TRASH_LOCATION, ITEMS);
	const trash = new FakeTrashClient({ count: ITEMS.length, ...info });
	const places = new FakePlacesClient({ places: fakePlaces('/home/test') });
	const h = await renderWorkspace(client, undefined, places, {
		sidebar: true,
		trash,
		ops: trash.opsClient(),
	});
	await screen.findByRole('button', { name: /^Home/ });
	return { ...h, trash, tree: client };
}

const placesGroup = () =>
	within(screen.getByRole('navigation', { name: 'Sidebar' })).getByRole('group', {
		name: 'Places',
	});
const trashPlace = () => within(placesGroup()).getByRole('button', { name: /^Trash/ });

async function openTrash(_h: Awaited<ReturnType<typeof setup>>) {
	fireEvent.click(trashPlace());
	await screen.findByRole('toolbar', { name: 'Trash actions' });
	await screen.findByRole('option', { name: /report\.pdf/ });
}

const row = (name: RegExp | string) => screen.getByRole('option', { name });
const toolbar = () => screen.getByRole('toolbar', { name: 'Trash actions' });
const button = (name: string) => within(toolbar()).getByRole('button', { name });
const status = () => screen.getByRole('group', { name: 'Status bar' });

describe('the Trash place', () => {
	it('is the last place and shows how many items the Trash holds', async () => {
		await setup();
		const places = within(placesGroup()).getAllByRole('button');
		expect(places[places.length - 1]).toBe(trashPlace());
		await waitFor(() => expect(trashPlace()).toHaveTextContent('3'));
		expect(within(trashPlace()).getByText('3 items in the Trash')).toBeInTheDocument();
	});

	it('shows no count for an empty Trash and says "1 item" for one', async () => {
		const h = await setup({ count: 0 });
		await waitFor(() => expect(h.trash.infoCalls).toBeGreaterThan(0));
		expect(trashPlace()).toHaveTextContent(/^Trash$/);
		h.trash.setInfo({ count: 1 });
		act(() => window.dispatchEvent(new Event('focus')));
		await waitFor(() => expect(within(trashPlace()).getByText('1 item in the Trash')).toBeTruthy());
	});

	it('updates when a job finishes', async () => {
		const h = await setup({ count: 2 });
		await waitFor(() => expect(trashPlace()).toHaveTextContent('2'));
		h.trash.setInfo({ count: 5 });
		h.trash.emit({
			kind: 'jobChanged',
			job: { ...jobShell(), state: { state: 'done' } },
			revision: 9,
		});
		await waitFor(() => expect(trashPlace()).toHaveTextContent('5'));
	});

	it('stays when the Trash cannot be browsed, marked and explained', async () => {
		await setup({ available: false, reason: 'the Trash portal can only move files to the trash' });
		await waitFor(() => expect(trashPlace()).toHaveAttribute('data-unavailable'));
		expect(trashPlace()).toHaveAttribute(
			'title',
			'the Trash portal can only move files to the trash',
		);
		expect(trashPlace()).not.toHaveTextContent(/\d/);
	});

	it('opens the Trash in the tab', async () => {
		const h = await setup();
		await openTrash(h);
		const snapshot = await h.tabs.getSnapshot();
		expect(snapshot.tabs.find((tab) => tab.id === snapshot.active)!.location.uri).toBe('trash:/');
		expect(trashPlace()).toHaveAttribute('aria-current', 'page');
	});

	it('has an Empty Trash item in its menu, off when the Trash is empty', async () => {
		const h = await setup({ count: 0 });
		await waitFor(() => expect(h.trash.infoCalls).toBeGreaterThan(0));
		fireEvent.contextMenu(trashPlace());
		const empty = await screen.findByRole('menuitem', { name: 'Empty Trash' });
		expect(empty).toHaveAttribute('aria-disabled', 'true');
		expect(screen.getByRole('menuitem', { name: 'Open' })).toBeInTheDocument();
		fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
	});

	it('asks before it empties the Trash from its menu', async () => {
		const h = await setup();
		await waitFor(() => expect(trashPlace()).toHaveTextContent('3'));
		fireEvent.contextMenu(trashPlace());
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Empty Trash' }));
		const dialog = await screen.findByRole('dialog', { name: 'Empty the Trash?' });
		expect(within(dialog).getByText(/All 3 items in the Trash/)).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Empty Trash' }));
		await waitFor(() => expect(h.trash.jobs).toHaveLength(1));
		expect(h.trash.last.request.kind).toEqual({ kind: 'emptyTrash', olderThanDays: null });
		expect(h.trash.last.request.sources).toEqual({ kind: 'locations', locations: [] });
	});
});

function jobShell() {
	return fakeJobSnapshot(7, { kind: 'restore' }, { state: 'queued' });
}

describe('the Trash view', () => {
	it('lists the items by their original names with where they were and when they were trashed', async () => {
		const h = await setup();
		await openTrash(h);
		const header = screen.getByRole('group', { name: 'Sort the list' });
		const columns = [...header.querySelectorAll('[data-column]')].map((node) =>
			node.getAttribute('data-column'),
		);
		expect(columns).toEqual(['name', 'original', 'deleted', 'size']);
		for (const label of ['Name', 'Original location', 'Date deleted', 'Size']) {
			expect(within(header).getByText(label)).toBeInTheDocument();
		}
		// Original location is where the item was, not a sort key.
		expect(within(header).queryByRole('button', { name: 'Original location' })).toBeNull();
		expect(within(header).queryByText('Modified')).toBeNull();
		expect(within(header).queryByText('Kind')).toBeNull();

		const report = row(/report\.pdf/);
		expect(report).toHaveTextContent('/home/test/docs');
		expect(report.querySelector('[data-column="original"]')).toHaveTextContent('/home/test/docs');
		expect(report.querySelector('[data-column="deleted"]')?.textContent).toMatch(/2023/);
		expect(row(/photo\.jpg/)).toHaveTextContent('/home/test/Pictures');
	});

	it('sorts by the date deleted, newest or oldest first', async () => {
		const h = await setup();
		await openTrash(h);
		const header = screen.getByRole('group', { name: 'Sort the list' });
		// Folders stay first; the files follow oldest first, then newest first.
		const names = () =>
			screen.getAllByRole('option').map((o) => o.querySelector('[class*="nameText"]')?.textContent);
		fireEvent.click(within(header).getByRole('button', { name: 'Date deleted' }));
		await waitFor(() => expect(names()).toEqual(['old drafts', 'photo.jpg', 'report.pdf']));
		fireEvent.click(within(header).getByRole('button', { name: /Date deleted/ }));
		await waitFor(() => expect(names()).toEqual(['old drafts', 'report.pdf', 'photo.jpg']));
	});

	it('says the Trash is empty rather than that a folder is', async () => {
		const h = await setup({ count: 0 });
		h.client.setFolder(TRASH_LOCATION, []);
		fireEvent.click(trashPlace());
		await screen.findByText('The Trash is empty.');
		expect(screen.queryByText('This folder is empty.')).toBeNull();
	});

	it('does not open an item on Enter or a double-click, and says so', async () => {
		const h = await setup();
		await openTrash(h);
		const list = screen.getByRole('listbox', { name: 'Files' });
		fireEvent.click(row(/report\.pdf/));
		fireEvent.keyDown(list, { key: 'Enter' });
		await waitFor(() =>
			expect(status()).toHaveTextContent(
				'Items in the Trash cannot be opened. Restore an item to open it.',
			),
		);
		fireEvent.doubleClick(row(/old drafts/));
		expect(h.client.opened).toEqual([]);
		// Still in the Trash: a folder did not navigate.
		const snapshot = await h.tabs.getSnapshot();
		expect(snapshot.tabs.find((tab) => tab.id === snapshot.active)!.location.uri).toBe('trash:/');
	});

	it('shows an unavailable Trash as an explanation, not a list', async () => {
		const h = await setup({ available: false });
		h.client.failOpening(TRASH_LOCATION, {
			kind: 'unsupported',
			what: 'the Trash portal can only move files to the trash',
		});
		fireEvent.click(trashPlace());
		const message = await screen.findByText('The Trash cannot be browsed here');
		expect(message).toBeInTheDocument();
		expect(
			screen.getByText('the Trash portal can only move files to the trash'),
		).toBeInTheDocument();
		expect(screen.queryByRole('listbox')).toBeNull();
	});
});

describe('the actions on the Trash', () => {
	it('turns Restore and Delete Permanently on only when something is selected', async () => {
		const h = await setup();
		await openTrash(h);
		expect(button('Restore')).toBeDisabled();
		expect(button('Delete Permanently')).toBeDisabled();
		expect(button('Empty Trash')).toBeEnabled();
		fireEvent.click(row(/report\.pdf/));
		expect(button('Restore')).toBeEnabled();
		expect(button('Delete Permanently')).toBeEnabled();
	});

	it('restores the selection as a job over the listing', async () => {
		const h = await setup();
		await openTrash(h);
		fireEvent.click(row(/report\.pdf/));
		fireEvent.click(button('Restore'));
		await waitFor(() => expect(h.trash.jobs).toHaveLength(1));
		const { request } = h.trash.last;
		expect(request.kind).toEqual({ kind: 'restore' });
		expect(request.sources).toMatchObject({ kind: 'selection', spec: { kind: 'some', ids: [1] } });
		// It reports the outcome in the status bar.
		h.trash.finish(h.trash.last.id, 1);
		await waitFor(() => expect(status()).toHaveTextContent('Restored 1 item'));
	});

	it('reports what was skipped and what failed', async () => {
		const h = await setup();
		await openTrash(h);
		fireEvent.click(row(/report\.pdf/));
		fireEvent.click(button('Restore'));
		await waitFor(() => expect(h.trash.jobs).toHaveLength(1));
		h.trash.move(
			h.trash.last.id,
			{
				state: 'failed',
				error: { kind: 'permissionDenied', location: { display: '/home/test/docs', uri: 'x' } },
				item: null,
				done: 0,
			},
			0,
		);
		await waitFor(() =>
			expect(status()).toHaveTextContent(
				'Could not restore: permission denied for /home/test/docs',
			),
		);
	});

	it('says so when the job cannot even be queued', async () => {
		const h = await setup();
		h.trash.rejectSubmits = { kind: 'queue', message: 'the queue is full' };
		await openTrash(h);
		fireEvent.click(row(/report\.pdf/));
		fireEvent.click(button('Restore'));
		await waitFor(() =>
			expect(status()).toHaveTextContent('Could not start the job: the queue is full'),
		);
	});

	it('asks before deleting for good, and does nothing when cancelled', async () => {
		const h = await setup();
		await openTrash(h);
		fireEvent.click(row(/report\.pdf/));
		fireEvent.click(button('Delete Permanently'));
		const dialog = await screen.findByRole('dialog', { name: 'Delete permanently?' });
		expect(within(dialog).getByText(/1 item will be deleted permanently/)).toBeInTheDocument();
		// Focus starts on the safe button.
		fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(h.trash.jobs).toEqual([]);

		fireEvent.click(button('Delete Permanently'));
		const again = await screen.findByRole('dialog', { name: 'Delete permanently?' });
		fireEvent.click(within(again).getByRole('button', { name: 'Delete Permanently' }));
		await waitFor(() => expect(h.trash.jobs).toHaveLength(1));
		expect(h.trash.last.request.kind).toEqual({ kind: 'delete' });
		h.trash.finish(h.trash.last.id, 1);
		await waitFor(() => expect(status()).toHaveTextContent('Deleted 1 item permanently'));
	});

	it('treats the Delete key as Delete Permanently, with the same question', async () => {
		const h = await setup();
		await openTrash(h);
		const list = screen.getByRole('listbox', { name: 'Files' });
		// Nothing selected: nothing to ask about.
		fireEvent.keyDown(list, { key: 'Delete' });
		expect(screen.queryByRole('dialog')).toBeNull();
		fireEvent.click(row(/report\.pdf/));
		fireEvent.keyDown(list, { key: 'Delete' });
		await screen.findByRole('dialog', { name: 'Delete permanently?' });
		expect(h.trash.jobs).toEqual([]);
	});

	it('counts every selected item in the question, including a select all', async () => {
		const h = await setup();
		await openTrash(h);
		const list = screen.getByRole('listbox', { name: 'Files' });
		fireEvent.click(row(/report\.pdf/));
		fireEvent.keyDown(list, { key: 'a', ctrlKey: true });
		fireEvent.click(button('Delete Permanently'));
		const dialog = await screen.findByRole('dialog', { name: 'Delete permanently?' });
		expect(within(dialog).getByText(/3 items will be deleted permanently/)).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Delete Permanently' }));
		await waitFor(() => expect(h.trash.jobs).toHaveLength(1));
		// The ids are written out, not left as "everything", so the job is exactly what was counted.
		const { sources } = h.trash.last.request;
		expect(sources).toMatchObject({ kind: 'selection', spec: { kind: 'some' } });
		expect(
			sources.kind === 'selection' ? [...sources.spec.ids].sort((a, b) => a - b) : null,
		).toEqual([1, 2, 3]);
	});

	it('deletes only what the question counted, not what the Trash gained while it was open', async () => {
		const h = await setup();
		await openTrash(h);
		const list = screen.getByRole('listbox', { name: 'Files' });
		fireEvent.click(row(/report\.pdf/));
		fireEvent.keyDown(list, { key: 'a', ctrlKey: true });
		fireEvent.click(button('Delete Permanently'));
		const dialog = await screen.findByRole('dialog', { name: 'Delete permanently?' });
		expect(within(dialog).getByText(/3 items will be deleted permanently/)).toBeInTheDocument();
		// Another program trashes something while the person reads the question.
		act(() => h.tree.addEntries(TRASH_LOCATION, [trashed(4, 'new arrival.txt', 0)]));
		await screen.findByRole('option', { name: /new arrival/ });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Delete Permanently' }));
		await waitFor(() => expect(h.trash.jobs).toHaveLength(1));
		const { sources } = h.trash.last.request;
		expect(sources).toMatchObject({ kind: 'selection', spec: { kind: 'some' } });
		expect(
			sources.kind === 'selection' ? [...sources.spec.ids].sort((a, b) => a - b) : null,
		).toEqual([1, 2, 3]);
	});

	it('empties the Trash after a confirmation', async () => {
		const h = await setup();
		await openTrash(h);
		fireEvent.click(button('Empty Trash'));
		const dialog = await screen.findByRole('dialog', { name: 'Empty the Trash?' });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Empty Trash' }));
		await waitFor(() => expect(h.trash.jobs).toHaveLength(1));
		h.trash.finish(h.trash.last.id, 3);
		await waitFor(() => expect(status()).toHaveTextContent('Emptied the Trash'));
	});
});

describe('the menus in the Trash', () => {
	it('offers Restore and Delete Permanently on an item, and never Open', async () => {
		const h = await setup();
		await openTrash(h);
		fireEvent.contextMenu(row(/report\.pdf/), { clientX: 10, clientY: 10 });
		const items = (await screen.findAllByRole('menuitem')).map((i) => i.textContent);
		expect(items).toHaveLength(2);
		expect(items[0]).toBe('Restore');
		expect(items[1]).toMatch(/^Delete Permanently/);
		expect(screen.queryByRole('menuitem', { name: /^Open/ })).toBeNull();
		fireEvent.click(screen.getByRole('menuitem', { name: 'Restore' }));
		await waitFor(() => expect(h.trash.jobs).toHaveLength(1));
		expect(h.trash.last.request.kind).toEqual({ kind: 'restore' });
	});

	it('offers Delete Permanently from the item menu with its question', async () => {
		const h = await setup();
		await openTrash(h);
		fireEvent.contextMenu(row(/photo\.jpg/), { clientX: 10, clientY: 10 });
		fireEvent.click(await screen.findByRole('menuitem', { name: /^Delete Permanently/ }));
		await screen.findByRole('dialog', { name: 'Delete permanently?' });
	});

	it('sorts by date deleted from the empty space, offers Empty Trash, and has no hidden-files toggle', async () => {
		const h = await setup();
		await openTrash(h);
		fireEvent.contextMenu(screen.getByRole('listbox', { name: 'Files' }).parentElement!, {
			clientX: 10,
			clientY: 10,
		});
		const menu = await screen.findByRole('menu');
		const labels = [...menu.querySelectorAll('[role^="menuitem"]')].map((i) => i.textContent);
		expect(labels).toContain('Date deleted');
		expect(labels).toContain('Empty Trash');
		expect(labels).not.toContain('Modified');
		expect(labels).not.toContain('Kind');
		expect(labels).not.toContain('Show hidden files');
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Empty Trash' }));
		await screen.findByRole('dialog', { name: 'Empty the Trash?' });
	});
});

describe('the questions a restore asks', () => {
	async function restoring() {
		const h = await setup();
		await openTrash(h);
		fireEvent.click(row(/report\.pdf/));
		fireEvent.click(button('Restore'));
		await waitFor(() => expect(h.trash.jobs).toHaveLength(1));
		return h;
	}

	it('asks a taken file name in the shared dialog, and says Keep both restores under a free name', async () => {
		const user = userEvent.setup();
		const h = await restoring();
		act(() => h.trash.waitForConflicts(h.trash.last.id, ['report.pdf'], 'fileOverFile'));
		const dialog = await screen.findByRole('dialog', {
			name: '1 item already exists in its original folder',
		});
		const choose = within(dialog).getByRole('combobox', { name: 'Choice for report.pdf' });
		await user.selectOptions(choose, 'Keep both');
		expect(within(dialog).getByText('Restored under a free name.')).toBeInTheDocument();
		await user.click(within(dialog).getByRole('button', { name: 'Continue' }));
		await waitFor(() =>
			expect(h.trash.last.resolutions).toEqual([
				{ source: { display: 'Trash/report.pdf', uri: 'trash:/report.pdf' }, policy: 'keepBoth' },
			]),
		);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
	});

	it('warns that replacing a folder replaces all of it', async () => {
		const user = userEvent.setup();
		const h = await restoring();
		act(() => h.trash.waitForConflicts(h.trash.last.id, ['old drafts'], 'folderOverFolder'));
		const dialog = await screen.findByRole('dialog', { name: /already exists in its original/ });
		await user.selectOptions(
			within(dialog).getByRole('combobox', { name: 'Choice for old drafts' }),
			'Replace',
		);
		expect(within(dialog).getByText(/Replaces the whole folder/)).toBeInTheDocument();
	});

	it('words several clashes together, answers them with one choice, and stops the job when cancelled', async () => {
		const user = userEvent.setup();
		const h = await restoring();
		act(() => h.trash.waitForConflicts(h.trash.last.id, ['a', 'b', 'c'], 'fileOverFile'));
		const dialog = await screen.findByRole('dialog', {
			name: '3 items already exist in their original folders',
		});
		await user.selectOptions(
			within(dialog).getByRole('combobox', { name: 'Apply to all remaining' }),
			'Skip',
		);
		await user.click(within(dialog).getByRole('checkbox', { name: /Apply to all conflicts/ }));
		await user.click(within(dialog).getByRole('button', { name: 'Continue' }));
		await waitFor(() => expect(h.trash.last.policies).toEqual(['skip']));

		act(() => h.trash.waitForConflicts(h.trash.last.id, ['d'], 'fileOverFile'));
		const again = await screen.findByRole('dialog', {
			name: '1 item already exists in its original folder',
		});
		await user.click(within(again).getByRole('button', { name: 'Cancel the operation' }));
		await waitFor(() => expect(h.trash.cancelled).toEqual([h.trash.last.id]));
	});

	it('offers to recreate a folder that is gone', async () => {
		const h = await restoring();
		act(() =>
			h.trash.move(h.trash.last.id, {
				state: 'waiting',
				reason: {
					kind: 'error',
					error: {
						kind: 'originMissingParent',
						location: { display: '/home/test/docs', uri: 'file:///home/test/docs' },
					},
					item: { display: 'Trash/1', uri: 'trash:/1' },
				},
			}),
		);
		const dialog = await screen.findByRole('dialog', { name: 'The original folder is gone' });
		expect(within(dialog).getByText(/\/home\/test\/docs no longer exists/)).toBeTruthy();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Recreate folders' }));
		await waitFor(() => expect(h.trash.last.decisions).toEqual(['createParents']));
	});

	it('lets a person skip any other error', async () => {
		const h = await restoring();
		act(() =>
			h.trash.move(h.trash.last.id, {
				state: 'waiting',
				reason: {
					kind: 'error',
					error: { kind: 'io', message: 'the disk hiccupped' },
					item: { display: 'Trash/1', uri: 'trash:/1' },
				},
			}),
		);
		const dialog = await screen.findByRole('dialog', { name: 'An item could not be processed' });
		expect(within(dialog).getByText(/the disk hiccupped/)).toBeInTheDocument();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Skip' }));
		await waitFor(() => expect(h.trash.last.decisions).toEqual(['skip']));
	});

	it('ignores another window’s job', async () => {
		const h = await setup();
		await openTrash(h);
		// A job this window did not start parks on a question: this window does not ask it.
		h.trash.emit({
			kind: 'jobAdded',
			job: { ...jobShell(), id: 99, originWindow: 'main-2', state: { state: 'queued' } },
			revision: 3,
		});
		act(() => {
			h.trash.emit({
				kind: 'jobChanged',
				job: {
					...jobShell(),
					id: 99,
					originWindow: 'main-2',
					state: {
						state: 'waiting',
						reason: { kind: 'conflicts', conflicts: [] },
					},
				},
				revision: 4,
			});
		});
		expect(screen.queryByRole('dialog')).toBeNull();
	});
});
