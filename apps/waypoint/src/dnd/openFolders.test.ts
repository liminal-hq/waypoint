// Verifies what a drop on + or a group chip opens: folders as tabs, a file's parent, a split pair
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { openListingModel } from '../browse/listingModel';
import { createListingSession } from '../browse/useListingSession';
import { selectIds } from '../browse/selection';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import { FOLDER } from '../test/browseHarness';
import type { OpenFoldersRequest } from './fileDrag';
import { foldersToOpen, MAX_DROP_TABS, openFolders, selectedEntries } from './openFolders';
import type { FileDragSource } from './fileDragModel';

const ENTRIES = [
	makeEntry(1, 'docs', { kind: 'directory' }),
	makeEntry(2, 'music', { kind: 'directory' }),
	makeEntry(3, 'notes.txt'),
	makeEntry(4, 'link', { kind: 'symlink', linkTarget: 'directory' }),
	makeEntry(5, 'oddity', { kind: 'symlink', linkTarget: 'file' }),
];

async function setup(selected: number[], options: { tabs?: number } = {}) {
	const vfs = new FakeVfsClient();
	vfs.setFolder(FOLDER, ENTRIES);
	const session = createListingSession(await openListingModel(vfs, FOLDER));
	session.store.getState().selectEntries(selected, 0);
	const api = new FakeTabsApi();
	await api.openTab(FOLDER);
	for (let i = 1; i < (options.tabs ?? 1); i++) await api.openTab(FOLDER);
	const source = {
		session,
		tab: 1,
		handle: session.model.handle,
		spec: { kind: 'some', ids: selected },
		count: selected.length,
		name: null,
		groups: ['folder'],
		folder: FOLDER,
		readOnly: false,
		rightButton: false,
	} as FileDragSource;
	const announce = vi.fn();
	// The deps read the newest snapshot; the test refreshes it after each change.
	let latest = await api.getSnapshot();
	const refresh = async () => (latest = await api.getSnapshot());
	return {
		vfs,
		session,
		api,
		source,
		announce,
		deps: { api, vfs, announce, snapshot: () => latest },
		refresh,
	};
}

const request = (
	source: FileDragSource,
	over: Partial<OpenFoldersRequest> = {},
): OpenFoldersRequest => ({
	source,
	target: 'plus',
	group: null,
	split: false,
	...over,
});

describe('foldersToOpen', () => {
	it('is each dropped folder, and a link to one', async () => {
		const h = await setup([1, 2, 4]);
		const folders = await foldersToOpen(request(h.source), h.vfs);
		expect(folders.map((folder) => folder.uri)).toEqual([
			'file:///home/test/docs',
			'file:///home/test/link',
			'file:///home/test/music',
		]);
	});

	it('is the folder the files are in when none of what was dropped is a folder', async () => {
		const h = await setup([3, 5]);
		expect(await foldersToOpen(request(h.source), h.vfs)).toEqual([FOLDER]);
	});

	it('opens only the folders when files are dropped with them', async () => {
		const h = await setup([1, 3]);
		expect(await foldersToOpen(request(h.source), h.vfs)).toEqual([
			fileLocation('/home/test/docs'),
		]);
	});

	it('stops at a limit of tabs', async () => {
		const vfs = new FakeVfsClient();
		vfs.setFolder(
			FOLDER,
			Array.from({ length: 20 }, (_, i) => makeEntry(i + 1, `d${i}`, { kind: 'directory' })),
		);
		const session = createListingSession(await openListingModel(vfs, FOLDER));
		session.store.getState().selectAll();
		const source = { session, handle: session.model.handle, folder: FOLDER } as FileDragSource;
		expect(await foldersToOpen(request(source), vfs)).toHaveLength(MAX_DROP_TABS);
	});
});

describe('selectedEntries', () => {
	it('finds the selected entries a page at a time, up to the limit', async () => {
		const h = await setup([1, 2, 3, 4]);
		const found = await selectedEntries(h.session.model, selectIds([2, 3, 4]), 2);
		expect(found.map((entry) => entry.name)).toEqual(['link', 'music']);
	});
});

describe('openFolders', () => {
	it('opens a new tab after the active one, brings the first to the front and says how many', async () => {
		const h = await setup([1, 2]);
		await openFolders(h.deps, request(h.source));
		const snapshot = await h.refresh();
		expect(snapshot.tabs.map((tab) => tab.location.uri)).toEqual([
			FOLDER.uri,
			'file:///home/test/docs',
			'file:///home/test/music',
		]);
		expect(snapshot.active).toBe(snapshot.tabs[1]!.id);
		expect(h.announce).toHaveBeenCalledWith('Opened 2 tabs');
	});

	it('opens a file’s parent folder', async () => {
		const h = await setup([3]);
		await openFolders(h.deps, request(h.source));
		expect((await h.refresh()).tabs).toHaveLength(2);
		expect(h.announce).toHaveBeenCalledWith('Opened 1 tab');
	});

	it('adds the new tab to the group a chip names', async () => {
		const h = await setup([1]);
		const group = await h.api.createGroup([1], 'Work');
		await openFolders(h.deps, request(h.source, { target: 'chip', group }));
		const snapshot = await h.refresh();
		const opened = snapshot.tabs.find((tab) => tab.location.uri.endsWith('/docs'))!;
		expect(opened.group).toBe(group);
	});

	it('pairs the active tab with a new tab at the first folder when Alt was held', async () => {
		const h = await setup([1, 2]);
		await openFolders(h.deps, request(h.source, { split: true }));
		const snapshot = await h.refresh();
		expect(snapshot.tabs).toHaveLength(2);
		expect(snapshot.pairs).toHaveLength(1);
		expect(snapshot.pairs[0]!.layout).toBe('sideBySide');
		expect(snapshot.active).toBe(snapshot.tabs[0]!.id);
	});
});
