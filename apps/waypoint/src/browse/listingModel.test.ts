// Verifies the listing cache: paged fetch, in-flight dedupe, stale replies, re-sorts and progress
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, fileLocation } from '../services/fakeVfsClient';
import { clientWith, FOLDER } from '../test/browseHarness';
import { ListingModel, openListingModel, PAGE_SIZE, toVfsError } from './listingModel';

async function open(count: number, latencyMs = 0) {
	const { client, entries } = clientWith(count, latencyMs);
	const getRange = vi.spyOn(client, 'getRange');
	const model = await openListingModel(client, FOLDER);
	return { client, entries, model, getRange };
}

/** Lets every pending microtask and zero-delay timer run. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('opening', () => {
	it('reports the listing the client opened', async () => {
		const { model } = await open(1000);
		expect(model.count).toBe(1000);
		expect(model.revision).toBe(1);
		expect(model.phase).toBe('ready');
		expect(model.sort).toEqual({ key: 'name', descending: false, directoriesFirst: true });
		expect(model.cachedCount).toBe(0);
		expect(model.entryAt(0)).toBeUndefined();
	});

	it('closes the listing when disposed, once', async () => {
		const { client, model } = await open(10);
		expect(client.openCount).toBe(1);
		model.dispose();
		model.dispose();
		expect(client.openCount).toBe(0);
	});

	it('rejects with the client error and leaves no listener behind', async () => {
		const client = new FakeVfsClient();
		await expect(openListingModel(client, fileLocation('/nope'))).rejects.toMatchObject({
			kind: 'notFound',
		});
	});

	it('coerces a foreign rejection into an io error', () => {
		expect(toVfsError(new Error('boom'))).toEqual({ kind: 'io', message: 'boom', location: null });
		expect(toVfsError({ kind: 'notFound' })).toEqual({ kind: 'notFound' });
	});
});

describe('paged fetch', () => {
	it('fetches the visible page and one either side, in pages of 256', async () => {
		const { model, getRange } = await open(2000);
		model.ensure(600, 620);
		await settle();
		expect(getRange.mock.calls.map((call) => call[1]).sort((a, b) => a - b)).toEqual([
			256, 512, 768,
		]);
		expect(getRange.mock.calls.every((call) => call[2] === PAGE_SIZE)).toBe(true);
		expect(model.entryAt(600)).toBeDefined();
		expect(model.entryAt(0)).toBeUndefined();
	});

	it('does not refetch pages it holds or has in flight', async () => {
		const { model, getRange } = await open(2000, 5);
		model.ensure(0, 10);
		model.ensure(0, 10);
		model.ensure(1, 12);
		await vi.waitFor(() => expect(model.entryAt(0)).toBeDefined());
		expect(getRange).toHaveBeenCalledTimes(2);
		model.ensure(0, 10);
		await settle();
		expect(getRange).toHaveBeenCalledTimes(2);
	});

	it('serves the same rows the client has, in view order', async () => {
		const { client, model } = await open(700);
		model.ensure(0, 699);
		await vi.waitFor(() => expect(model.entryAt(699)).toBeDefined());
		const truth = await client.getRange(model.handle, 0, 700);
		expect(Array.from({ length: 700 }, (_, i) => model.entryAt(i))).toEqual(truth);
	});

	it('never fetches for an empty listing', async () => {
		const { model, getRange } = await open(0);
		model.ensure(0, 10);
		await settle();
		expect(getRange).not.toHaveBeenCalled();
	});

	it('reads a range across pages on demand', async () => {
		const { client, model } = await open(1000);
		const rows = await model.readRange(250, 520);
		const truth = await client.getRange(model.handle, 250, 270);
		expect(rows).toEqual(truth);
		expect(await model.idAt(300)).toBe(truth[50]!.id);
	});

	it('clamps a range read to the listing', async () => {
		const { model } = await open(10);
		expect(await model.readRange(5, 500)).toHaveLength(5);
	});

	it('keeps the cache bounded by dropping pages far from the viewport', async () => {
		const { model } = await open(20_000);
		for (let first = 0; first < 20_000; first += 1000) {
			model.ensure(first, first + 20);
			await settle();
		}
		expect(model.cachedCount).toBeLessThan(60 * PAGE_SIZE);
		expect(model.entryAt(19_010)).toBeDefined();
		expect(model.entryAt(0)).toBeUndefined();
	});
});

describe('stale replies', () => {
	it('drops a page that was fetched before a newer revision and refetches it', async () => {
		const { client, entries, model, getRange } = await open(600, 20);
		model.ensure(0, 10);
		// The folder changes while the first pages are in flight.
		client.removeEntries(FOLDER, [entries[5]!.id]);
		expect(model.revision).toBe(2);
		await vi.waitFor(() => expect(model.entryAt(0)).toBeDefined());
		// The first pair of replies were for revision 1 and were dropped, so the pages went again.
		expect(getRange.mock.calls.length).toBeGreaterThan(2);
		await vi.waitFor(() => expect(model.staleCount + (model.hasFresh(9) ? 0 : 1)).toBe(0));
		const truth = await client.getRange(model.handle, 0, 599);
		expect(Array.from({ length: 10 }, (_, i) => model.entryAt(i))).toEqual(truth.slice(0, 10));
		expect(model.staleCount).toBe(0);
	});

	it('refetches a page whose fetch rejected after the revision moved on', async () => {
		const { client, entries, model, getRange } = await open(10);
		getRange.mockRejectedValueOnce({ kind: 'io', message: 'late', location: null });
		model.ensure(0, 5);
		client.removeEntries(FOLDER, [entries[5]!.id]);
		await vi.waitFor(() => expect(model.hasFresh(0)).toBe(true));
		expect(model.error).toBeNull();
	});

	it('ignores events from older revisions and other listings', async () => {
		const { model } = await open(10);
		model.applyEvent({ kind: 'changed', handle: model.handle, revision: 1, count: 99, ops: [] });
		expect(model.count).toBe(10);
		model.applyEvent({ kind: 'changed', handle: 999, revision: 50, count: 99, ops: [] });
		expect(model.count).toBe(10);
		model.applyEvent({
			kind: 'progress',
			handle: model.handle,
			revision: 0,
			phase: 'scanning',
			scanned: 5,
			count: 5,
		});
		expect(model.phase).toBe('ready');
	});
});

describe('sorting', () => {
	it('keeps old rows on screen, stale, until the new order is fetched', async () => {
		const { client, model } = await open(600);
		model.ensure(0, 10);
		await settle();
		const before = model.entryAt(0);
		await model.setSort({ key: 'name', descending: true, directoriesFirst: true });
		expect(model.sort.descending).toBe(true);
		expect(model.revision).toBe(2);
		// No blank flash: the previous entry is still there, marked stale.
		expect(model.entryAt(0)).toEqual(before);
		expect(model.hasFresh(0)).toBe(false);
		model.ensure(0, 10);
		await settle();
		const truth = await client.getRange(model.handle, 0, 10);
		expect(Array.from({ length: 10 }, (_, i) => model.entryAt(i))).toEqual(truth);
		expect(model.entryAt(0)).not.toEqual(before);
	});

	it('keeps the confirmed sort when an event already moved the revision on', async () => {
		const { client, model } = await open(10);
		const real = client.setSort.bind(client);
		vi.spyOn(client, 'setSort').mockImplementation(async (handle, sort) => {
			const snapshot = await real(handle, sort);
			// An event for a later revision lands before the reply is handled.
			model.applyEvent({
				kind: 'changed',
				handle,
				revision: snapshot.revision + 1,
				count: 10,
				ops: [],
			});
			return snapshot;
		});
		await model.setSort({ key: 'name', descending: true, directoriesFirst: true });
		expect(model.sort.descending).toBe(true);
		expect(model.revision).toBe(3);
		expect(model.hasFresh(0)).toBe(false);
	});

	it('reports a failed sort as the model error', async () => {
		const { client, model } = await open(10);
		vi.spyOn(client, 'setSort').mockRejectedValue({ kind: 'io', message: 'no', location: null });
		await model.setSort({ key: 'size', descending: false, directoriesFirst: true });
		expect(model.error).toEqual({ kind: 'io', message: 'no', location: null });
	});
});

describe('progress and failure', () => {
	it('tracks the scan phase and a growing count', async () => {
		const { model } = await open(0);
		model.applyEvent({
			kind: 'progress',
			handle: model.handle,
			revision: 1,
			phase: 'scanning',
			scanned: 800,
			count: 800,
		});
		expect(model.phase).toBe('scanning');
		expect(model.scanned).toBe(800);
		expect(model.count).toBe(800);
		model.applyEvent({
			kind: 'progress',
			handle: model.handle,
			revision: 2,
			phase: 'ready',
			scanned: 1000,
			count: 1000,
		});
		expect(model.phase).toBe('ready');
		expect(model.count).toBe(1000);
	});

	it('records a failed listing', async () => {
		const { client, model } = await open(10);
		client.failListing(model.handle, { kind: 'permissionDenied', location: FOLDER });
		expect(model.error).toMatchObject({ kind: 'permissionDenied' });
		expect(model.phase).toBe('failed');
	});

	it('does not treat a closed handle as an error', async () => {
		const { client, model } = await open(600);
		await client.closeListing(model.handle);
		model.ensure(0, 10);
		await settle();
		expect(model.error).toBeNull();
	});

	it('replays events that arrived before the snapshot', async () => {
		const client = new FakeVfsClient();
		client.setFolder(FOLDER, []);
		const model = await openListingModel(client, FOLDER);
		expect(model).toBeInstanceOf(ListingModel);
		expect(model.phase).toBe('ready');
	});

	it('notifies subscribers with a changing version', async () => {
		const { client, model } = await open(10);
		const listener = vi.fn();
		const off = model.subscribe(listener);
		const before = model.getVersion();
		client.failListing(model.handle, { kind: 'cancelled' });
		client.failListing(model.handle, { kind: 'io', message: 'x', location: null });
		expect(listener).toHaveBeenCalled();
		expect(model.getVersion()).toBeGreaterThan(before);
		off();
	});
});
