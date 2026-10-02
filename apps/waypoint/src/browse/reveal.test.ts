// Verifies finding the entries a command made: by name, by what the listing gained, and by selection
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { FOLDER } from '../test/browseHarness';
import { openListingModel } from './listingModel';
import {
	findByName,
	firstSelectedNames,
	InsertTracker,
	revealByName,
	revealInserted,
	waitForInserts,
} from './reveal';
import { everything, selectIds } from './selection';
import { createListingSession } from './useListingSession';

async function open(names: string[]) {
	const vfs = new FakeVfsClient();
	vfs.setFolder(
		FOLDER,
		names.map((name, index) => makeEntry(index + 1, name)),
	);
	const model = await openListingModel(vfs, FOLDER);
	return { vfs, session: createListingSession(model), model };
}

const disposables: Array<() => void> = [];
afterEach(() => {
	while (disposables.length) disposables.pop()!();
});

describe('findByName', () => {
	it('finds the position of an exact name across pages, and nothing for a near miss', async () => {
		const names = Array.from({ length: 600 }, (_, i) => `f${String(i).padStart(4, '0')}`);
		const { model } = await open(names);
		expect(await findByName(model, 'f0000')).toBe(0);
		expect(await findByName(model, 'f0599')).toBe(599);
		expect(await findByName(model, 'f059')).toBeNull();
		expect(await findByName(model, 'F0000')).toBeNull();
	});
});

describe('revealByName', () => {
	it('selects, focuses, scrolls to and (when asked) renames an entry already there', async () => {
		const { session, model } = await open(['a', 'b', 'c']);
		expect(await revealByName(model, session.store, 'b', { rename: true })).toBe(true);
		const state = session.store.getState();
		expect(state.selection).toEqual(selectIds([2]));
		expect(state.focus).toBe(1);
		expect(state.scrollRequest?.position).toBe(1);
		expect(state.renaming).toBe(2);
	});

	it('waits for an entry the listing has not shown yet', async () => {
		const { vfs, session, model } = await open(['a']);
		const found = revealByName(model, session.store, 'new', { timeoutMs: 2000 });
		setTimeout(() => vfs.addEntries(FOLDER, [makeEntry(9, 'new')]), 10);
		expect(await found).toBe(true);
		expect(session.store.getState().selection).toEqual(selectIds([9]));
		expect(session.store.getState().renaming).toBeNull();
	});

	it('gives up, without a rename, when the entry never appears', async () => {
		const { session, model } = await open(['a']);
		expect(await revealByName(model, session.store, 'nope', { rename: true, timeoutMs: 40 })).toBe(
			false,
		);
		expect(session.store.getState().renaming).toBeNull();
	});
});

describe('InsertTracker', () => {
	it('follows the entries a listing gains and selects them all', async () => {
		const { vfs, session, model } = await open(['a', 'd']);
		const tracker = new InsertTracker(model);
		disposables.push(() => tracker.dispose());
		vfs.addEntries(FOLDER, [makeEntry(10, 'b'), makeEntry(11, 'c')]);
		await waitForInserts(model, tracker, 2, 1000);
		expect(tracker.count).toBe(2);
		expect(await revealInserted(session.store, tracker)).toBe(2);
		const state = session.store.getState();
		expect(state.selection).toEqual(selectIds([10, 11]));
		expect(state.focus).toBe(1);
	});

	it('keeps following an insert through a later patch that moves it', async () => {
		const { vfs, model } = await open(['m']);
		const tracker = new InsertTracker(model);
		disposables.push(() => tracker.dispose());
		vfs.addEntries(FOLDER, [makeEntry(10, 'n')]);
		await waitForInserts(model, tracker, 1, 1000);
		vfs.addEntries(FOLDER, [makeEntry(11, 'a')]); // lands above, pushing `n` down
		await new Promise((resolve) => setTimeout(resolve, 20));
		const found = await tracker.entries();
		expect(found.map(({ entry }) => entry.name)).toEqual(['a', 'n']);
	});

	it('loses what it follows when the listing is reset', async () => {
		const { vfs, model } = await open(['a']);
		const tracker = new InsertTracker(model);
		disposables.push(() => tracker.dispose());
		vfs.addEntries(FOLDER, [makeEntry(10, 'b')]);
		await waitForInserts(model, tracker, 1, 1000);
		await model.setSort({ key: 'name', descending: true, directoriesFirst: true });
		expect(tracker.count).toBe(0);
		expect(await tracker.entries()).toEqual([]);
	});
});

describe('firstSelectedNames', () => {
	it('lists the names of selected entries from the top, up to a limit', async () => {
		const { model } = await open(['a', 'b', 'c', 'd', 'e']);
		expect(await firstSelectedNames(model, selectIds([2, 4, 5]), 2)).toEqual(['b', 'd']);
		expect(await firstSelectedNames(model, everything, 3)).toEqual(['a', 'b', 'c']);
		expect(await firstSelectedNames(model, { kind: 'allExcept', ids: new Set([1, 2]) }, 2)).toEqual(
			['c', 'd'],
		);
	});
});
