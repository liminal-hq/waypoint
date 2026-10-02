// Tests what the Shelf does: add, remove, open, reveal, copy, the lazy check for missing files and the tidy-up after a move
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ShelfItem } from '@liminal-hq/waypoint-protocol/generated/ShelfItem';
import { describe, expect, it, vi } from 'vitest';
import { openListingModel } from '../browse/listingModel';
import { createListingSession } from '../browse/useListingSession';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { SHELF_LIMIT } from '../services/fakeTabsStore';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import { FOLDER } from '../test/browseHarness';
import { createShelfActions, type ShelfActions } from './shelfActions';
import { createShelfStore, type ShelfStore } from './shelfStore';

const DOCS = fileLocation('/home/test/docs');

interface Harness {
	api: FakeTabsApi;
	vfs: FakeVfsClient;
	store: ShelfStore;
	actions: ShelfActions;
	said: string[];
	announced: string[];
	written: string[];
	clipboard: { setFromLocations: ReturnType<typeof vi.fn> };
	items(): ShelfItem[];
	/** Pushes the session's Shelf into the store, as the provider does. */
	sync(): Promise<void>;
}

async function setup(): Promise<Harness> {
	const api = new FakeTabsApi();
	const vfs = new FakeVfsClient();
	vfs.setFolder(FOLDER, [
		makeEntry(1, 'docs', { kind: 'directory' }),
		makeEntry(2, 'a.txt'),
		makeEntry(3, 'b.txt'),
	]);
	vfs.setFolder(DOCS, [makeEntry(1, 'inner.txt')]);
	const store = createShelfStore();
	const said: string[] = [];
	const announced: string[] = [];
	const written: string[] = [];
	const clipboard = { setFromLocations: vi.fn(async () => ({})) };
	const actions = createShelfActions({
		store,
		api,
		vfs,
		say: (text) => void said.push(text),
		announce: (text) => void announced.push(text),
		activeTab: () => 1,
		clipboard: () => clipboard as never,
		writeText: async (text) => void written.push(text),
	});
	const sync = async () => {
		const snapshot = await api.getSnapshot();
		store.getState().sync(snapshot.shelf, snapshot.revision);
	};
	return {
		api,
		vfs,
		store,
		actions,
		said,
		announced,
		written,
		clipboard,
		items: () => store.getState().items as ShelfItem[],
		sync,
	};
}

describe('add', () => {
	it('puts locations on the Shelf and says how many were new', async () => {
		const h = await setup();
		expect(
			await h.actions.add([fileLocation('/home/test/a.txt'), fileLocation('/home/test/b.txt')]),
		).toBe(2);
		expect(h.said).toEqual(['Added 2 items to the Shelf']);
		await h.sync();
		expect(await h.actions.add([fileLocation('/home/test/a.txt')])).toBe(0);
		expect(h.said[1]).toBe('Already on the Shelf');
	});

	it('says the Shelf is full, and adds nothing', async () => {
		const h = await setup();
		await h.api.addToShelf(
			Array.from({ length: SHELF_LIMIT }, (_, i) => fileLocation(`/home/test/f${i}`)),
		);
		await h.sync();
		expect(await h.actions.add([fileLocation('/home/test/extra')])).toBe(0);
		expect(h.said).toEqual([`The Shelf is full: it holds at most ${SHELF_LIMIT} items`]);
		expect((await h.api.getSnapshot()).shelf).toHaveLength(SHELF_LIMIT);
	});

	it('puts the selection of a listing on the Shelf', async () => {
		const h = await setup();
		const session = createListingSession(await openListingModel(h.vfs, FOLDER));
		session.store.getState().click(1, 2);
		session.store.getState().toggleAt(2, 3);
		expect(await h.actions.addSelection(session)).toBe(2);
		await h.sync();
		expect(h.items().map((i) => i.location.display)).toEqual([
			'/home/test/a.txt',
			'/home/test/b.txt',
		]);
		expect(h.items()[0]?.origin.display).toBe('/home/test');
	});

	it('adds nothing for an empty selection', async () => {
		const h = await setup();
		const session = createListingSession(await openListingModel(h.vfs, FOLDER));
		expect(await h.actions.addSelection(session)).toBe(0);
		expect(h.said).toEqual([]);
	});
});

describe('remove and clear', () => {
	it('takes items off and announces it, never touching a file', async () => {
		const h = await setup();
		await h.api.addToShelf([fileLocation('/home/test/a.txt'), fileLocation('/home/test/b.txt')]);
		await h.sync();
		await h.actions.remove([h.items()[0]!.id]);
		expect(h.announced).toEqual(['Removed 1 item from the Shelf']);
		await h.sync();
		expect(h.items()).toHaveLength(1);
		await h.actions.clear();
		expect(h.announced[1]).toBe('Cleared the Shelf');
		await h.sync();
		expect(h.items()).toHaveLength(0);
	});

	it('says nothing for ids that are not there', async () => {
		const h = await setup();
		await h.actions.remove([99]);
		expect(h.announced).toEqual([]);
	});
});

describe('open and reveal', () => {
	it('opens a folder in the active tab, once known to be a folder', async () => {
		const h = await setup();
		const tab = await h.api.openTab(FOLDER);
		void tab;
		await h.api.addToShelf([DOCS]);
		await h.sync();
		const [item] = h.items();
		await h.actions.check(h.items());
		const navigate = vi.spyOn(h.api, 'navigate');
		await h.actions.open(item!);
		expect(navigate).toHaveBeenCalledWith(1, DOCS);
	});

	it('shows a file in its folder in a new tab with the file focused', async () => {
		const h = await setup();
		await h.api.openTab(FOLDER);
		await h.api.addToShelf([fileLocation('/home/test/a.txt')]);
		await h.sync();
		await h.actions.open(h.items()[0]!);
		const snapshot = await h.api.getSnapshot();
		const opened = snapshot.tabs[snapshot.tabs.length - 1]!;
		expect(opened.location).toEqual(FOLDER);
		expect(opened.hints.focused).toBe('a.txt');
		expect(snapshot.active).toBe(opened.id);
	});
});

describe('copying', () => {
	it('puts the display paths on the text clipboard, one per line', async () => {
		const h = await setup();
		await h.api.addToShelf([fileLocation('/home/test/a.txt'), fileLocation('/home/test/b.txt')]);
		await h.sync();
		await h.actions.copyPath(h.items());
		expect(h.written).toEqual(['/home/test/a.txt\n/home/test/b.txt']);
	});

	it('puts the files on the shared clipboard as a copy and says so', async () => {
		const h = await setup();
		await h.api.addToShelf([fileLocation('/home/test/a.txt')]);
		await h.sync();
		await h.actions.copyFiles(h.items());
		expect(h.clipboard.setFromLocations).toHaveBeenCalledWith(
			[fileLocation('/home/test/a.txt')],
			'copy',
		);
		expect(h.said).toEqual(['Copied 1 item from the Shelf']);
	});
});

describe('the check for missing files', () => {
	it('marks folders, files and missing ones, and asks only about what it has not yet', async () => {
		const h = await setup();
		await h.api.addToShelf([
			DOCS,
			fileLocation('/home/test/a.txt'),
			fileLocation('/home/test/gone.txt'),
		]);
		await h.sync();
		const spy = vi.spyOn(h.vfs, 'checkFolder');
		await h.actions.check(h.items());
		const status = h.store.getState().status;
		expect(status.get(DOCS.uri)).toBe('folder');
		expect(status.get('file:///home/test/a.txt')).toBe('file');
		expect(status.get('file:///home/test/gone.txt')).toBe('missing');
		expect(spy).toHaveBeenCalledTimes(3);
		await h.actions.check(h.items());
		expect(spy).toHaveBeenCalledTimes(3);
		await h.actions.check(h.items(), { force: true });
		expect(spy).toHaveBeenCalledTimes(6);
	});

	it('leaves a file it could not look at unmarked, rather than calling it missing', async () => {
		const h = await setup();
		h.vfs.failOpening(fileLocation('/home/test/locked'), {
			kind: 'permissionDenied',
			location: fileLocation('/home/test/locked'),
		});
		await h.api.addToShelf([fileLocation('/home/test/locked')]);
		await h.sync();
		await h.actions.check(h.items());
		expect(h.store.getState().status.size).toBe(0);
	});

	it('does not run two looks at once, and still looks at what was asked for while one ran', async () => {
		const h = await setup();
		await h.api.addToShelf([fileLocation('/home/test/a.txt')]);
		await h.sync();
		const real = h.vfs.checkFolder.bind(h.vfs);
		const spy = vi.spyOn(h.vfs, 'checkFolder');
		let open = 0;
		let most = 0;
		spy.mockImplementation(async (location) => {
			open += 1;
			most = Math.max(most, open);
			for (let turn = 0; turn < 5; turn++) await Promise.resolve();
			open -= 1;
			return real(location);
		});
		const first = h.actions.check(h.items());
		await Promise.resolve();
		await Promise.resolve();
		const second = h.actions.check(h.items(), { force: true });
		await Promise.all([first, second]);
		// The forced look was kept and ran after the first, never beside it.
		expect(spy).toHaveBeenCalledTimes(2);
		expect(most).toBe(1);
	});

	it('gives items added while a look runs their status, instead of leaving them unknown', async () => {
		const h = await setup();
		await h.api.addToShelf([fileLocation('/home/test/a.txt')]);
		await h.sync();
		const first = h.actions.check(h.items());
		// A file is added (and has already moved away) while the first look is under way.
		await h.api.addToShelf([fileLocation('/home/test/gone.txt')]);
		await h.sync();
		const second = h.actions.check(h.items());
		await Promise.all([first, second]);
		const status = h.store.getState().status;
		expect(status.get('file:///home/test/a.txt')).toBe('file');
		expect(status.get('file:///home/test/gone.txt')).toBe('missing');
	});
});

describe('after a move', () => {
	it('removes the items whose files left and keeps the ones that did not', async () => {
		const h = await setup();
		await h.api.addToShelf([
			fileLocation('/home/test/a.txt'),
			fileLocation('/home/test/b.txt'),
			fileLocation('/home/test/c.txt'),
		]);
		await h.sync();
		// a.txt left (the fake no longer lists it); b.txt stayed (a conflict skipped it).
		h.vfs.setFolder(FOLDER, [makeEntry(3, 'b.txt'), makeEntry(4, 'c.txt')]);
		await h.actions.afterMove([fileLocation('/home/test/a.txt'), fileLocation('/home/test/b.txt')]);
		await h.sync();
		expect(h.items().map((i) => i.name)).toEqual(['b.txt', 'c.txt']);
	});

	it('does nothing for no locations', async () => {
		const h = await setup();
		const remove = vi.spyOn(h.api, 'removeFromShelf');
		await h.actions.afterMove([]);
		expect(remove).not.toHaveBeenCalled();
	});
});
