// Verifies the per-listing store: click, toggle and range selection by view position, focus, and patches
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { makeEntry } from '../services/fakeVfsClient';
import { clientWith, FOLDER } from '../test/browseHarness';
import { createBrowseStore } from './browseStore';
import { openListingModel } from './listingModel';
import { isSelected, selectedCount } from './selection';

async function setup(count = 1000) {
	const { client } = clientWith(count);
	const model = await openListingModel(client, FOLDER);
	const store = createBrowseStore(model);
	const ids = async (from: number, to: number) =>
		(await model.readRange(from, to)).map((e) => e.id);
	const selected = () => selectedCount(store.getState().selection, model.count);
	return { client, model, store, ids, selected };
}

describe('click and toggle', () => {
	it('replaces the selection on click and moves the anchor and focus', async () => {
		const { model, store } = await setup();
		const [a, b] = await model.readRange(3, 5);
		store.getState().click(3, a!.id);
		store.getState().click(4, b!.id);
		const state = store.getState();
		expect(isSelected(state.selection, a!.id)).toBe(false);
		expect(isSelected(state.selection, b!.id)).toBe(true);
		expect(state.anchor).toBe(4);
		expect(state.focus).toBe(4);
		expect(state.touched).toBe(true);
	});

	it('toggles an entry without disturbing the rest', async () => {
		const { store, ids, selected } = await setup();
		const [a, b, c] = await ids(0, 3);
		store.getState().click(0, a!);
		store.getState().toggleAt(1, b!);
		store.getState().toggleAt(2, c!);
		expect(selected()).toBe(3);
		store.getState().toggleAt(1, b!);
		expect(selected()).toBe(2);
		expect(isSelected(store.getState().selection, b!)).toBe(false);
	});
});

describe('ranges', () => {
	it('selects from the anchor to a later position, in view positions', async () => {
		const { store, ids, selected } = await setup();
		const all = await ids(10, 16);
		store.getState().click(10, all[0]!);
		await store.getState().extendTo(15);
		expect(selected()).toBe(6);
		for (const id of all) expect(isSelected(store.getState().selection, id)).toBe(true);
		expect(store.getState().anchor).toBe(10);
		expect(store.getState().focus).toBe(15);
	});

	it('selects backwards and across a page boundary', async () => {
		const { store, ids, selected } = await setup();
		const start = await ids(300, 301);
		store.getState().click(300, start[0]!);
		await store.getState().extendTo(250);
		expect(selected()).toBe(51);
	});

	it('shrinks the range when the target comes back toward the anchor', async () => {
		const { store, ids, selected } = await setup();
		store.getState().click(5, (await ids(5, 6))[0]!);
		await store.getState().extendTo(20);
		await store.getState().extendTo(8);
		expect(selected()).toBe(4);
	});

	it('adds a range to the existing selection when additive', async () => {
		const { store, ids, selected } = await setup();
		store.getState().click(0, (await ids(0, 1))[0]!);
		store.getState().toggleAt(50, (await ids(50, 51))[0]!);
		await store.getState().extendTo(54, true);
		expect(selected()).toBe(6);
		expect(isSelected(store.getState().selection, (await ids(0, 1))[0]!)).toBe(true);
	});

	it('treats a range over the whole listing as select all, without reading it', async () => {
		const { store, model, selected } = await setup(100_000);
		store.getState().click(0, (await model.readRange(0, 1))[0]!.id);
		await store.getState().extendTo(99_999);
		expect(store.getState().selection.kind).toBe('allExcept');
		expect(selected()).toBe(100_000);
		// Only the one page for the click was ever fetched.
		expect(model.cachedCount).toBeLessThan(600);
	});

	it('starts from the focus when there is no anchor', async () => {
		const { store, selected } = await setup();
		store.getState().moveTo(4, false);
		await store.getState().extendTo(6);
		expect(selected()).toBe(3);
	});

	it('lets the newest action win over a slower range read', async () => {
		const { store, ids, selected } = await setup();
		store.getState().click(0, (await ids(0, 1))[0]!);
		const slow = store.getState().extendTo(700);
		store.getState().click(2, (await ids(2, 3))[0]!);
		await slow;
		expect(selected()).toBe(1);
	});
});

describe('focus', () => {
	it('moves the focus and selects the entry there', async () => {
		const { store, ids, selected } = await setup();
		expect(store.getState().moveTo(7, true)).toBe(7);
		await Promise.resolve();
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(selected()).toBe(1);
		expect(isSelected(store.getState().selection, (await ids(7, 8))[0]!)).toBe(true);
	});

	it('moves the focus alone without selecting', async () => {
		const { store, selected } = await setup();
		store.getState().moveTo(7, false);
		expect(store.getState().focus).toBe(7);
		expect(selected()).toBe(0);
	});

	it('clamps to the listing and does nothing in an empty one', async () => {
		const { store } = await setup(10);
		expect(store.getState().moveTo(-5, false)).toBe(0);
		expect(store.getState().moveTo(500, false)).toBe(9);
		const empty = await setup(0);
		expect(empty.store.getState().moveTo(3, true)).toBeNull();
	});

	it('toggles the focused entry', async () => {
		const { store, selected } = await setup();
		store.getState().moveTo(2, false);
		store.getState().toggleFocused();
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(selected()).toBe(1);
		store.getState().toggleFocused();
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(selected()).toBe(0);
	});

	it('clamps a focus left past the end when the listing shrinks', async () => {
		const { client, store } = await setup(100);
		store.getState().moveTo(90, false);
		client.setFolder(FOLDER, []);
		expect(store.getState().focus).toBeNull();
	});
});

describe('select all, invert and deselect', () => {
	it('selects everything without listing the ids', async () => {
		// The claim is about the representation, not speed: "all" is a flag and an empty id set, so
		// the cost does not grow with the listing. 50 000 rows show that without the seconds a
		// 500 000-entry fake takes to build and sort on a slow CI runner.
		const { store, selected } = await setup(50_000);
		store.getState().selectAll();
		expect(selected()).toBe(50_000);
		expect(store.getState().selection.ids.size).toBe(0);
	});

	it('inverts and deselects', async () => {
		const { store, ids, selected } = await setup(100);
		store.getState().click(0, (await ids(0, 1))[0]!);
		store.getState().invertSelection();
		expect(selected()).toBe(99);
		store.getState().invertSelection();
		expect(selected()).toBe(1);
		store.getState().deselectAll();
		expect(selected()).toBe(0);
		expect(store.getState().touched).toBe(true);
	});
});

describe('following patches', () => {
	it('drops selected ids that were removed and keeps the rest', async () => {
		const { client, model, store, ids, selected } = await setup(100);
		const [a, b, c] = await ids(10, 13);
		store.getState().click(10, a!);
		store.getState().toggleAt(11, b!);
		store.getState().toggleAt(12, c!);
		client.removeEntries(FOLDER, [b!]);
		expect(model.count).toBe(99);
		expect(selected()).toBe(2);
		expect(isSelected(store.getState().selection, a!)).toBe(true);
		expect(isSelected(store.getState().selection, b!)).toBe(false);
		expect(isSelected(store.getState().selection, c!)).toBe(true);
	});

	it('keeps the selection when entries are inserted above it', async () => {
		const { client, model, store, ids } = await setup(100);
		const [a] = await ids(40, 41);
		store.getState().click(40, a!);
		client.addEntries(FOLDER, [
			makeEntry(5000, '0000-first.txt'),
			makeEntry(5001, '0000-second.txt'),
		]);
		expect(isSelected(store.getState().selection, a!)).toBe(true);
		expect(store.getState().focus).toBe(42);
		expect(model.count).toBe(102);
	});

	it('moves the focus up when entries above it are removed', async () => {
		const { client, model, store, ids } = await setup(100);
		const gone = await ids(0, 3);
		store.getState().moveTo(20, false);
		await model.readRange(0, 30);
		client.removeEntries(FOLDER, gone);
		expect(store.getState().focus).toBe(17);
	});

	it('keeps select-all across inserts and removals', async () => {
		const { client, store, selected } = await setup(100);
		store.getState().selectAll();
		client.addEntries(FOLDER, [makeEntry(9000, 'new.txt')]);
		expect(selected()).toBe(101);
		client.removeEntries(FOLDER, [9000]);
		expect(selected()).toBe(100);
	});
});
