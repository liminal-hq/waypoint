// Verifies one listing per tab: opened when a tab shows a folder, released when it leaves or closes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, fileLocation, syntheticEntries } from '../services/fakeVfsClient';
import { ListingManager } from './listingManager';

const A = fileLocation('/a');
const B = fileLocation('/b');

function tab(id: number, location = A): TabSnapshot {
	return { id, location, back: [], forward: [] };
}

function setup(options = {}) {
	const client = new FakeVfsClient();
	client.setFolder(A, syntheticEntries(600));
	client.setFolder(B, syntheticEntries(5));
	return { client, manager: new ListingManager(client, options) };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

afterEach(() => vi.useRealTimers());

describe('ListingManager', () => {
	it('opens the active tab only, and shows opening before ready', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1), tab(2, B)], 1);
		expect(manager.stateFor(1)).toEqual({ status: 'opening' });
		expect(manager.stateFor(2)).toBeUndefined();
		await settle();
		expect(manager.stateFor(1)?.status).toBe('ready');
		expect(client.openCount).toBe(1);
	});

	it('closes the old listing when the tab navigates and inherits its sort', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1)], 1);
		await settle();
		const first = manager.stateFor(1);
		if (first?.status !== 'ready') throw new Error('not ready');
		await first.session.model.setSort({ key: 'size', descending: true, directoriesFirst: true });

		manager.sync([tab(1, B)], 1);
		await settle();
		expect(client.openCount).toBe(1);
		const next = manager.stateFor(1);
		expect(next?.status === 'ready' && next.session.model.sort.key).toBe('size');
	});

	it('does not reopen for a new object at the same location', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1)], 1);
		await settle();
		const before = manager.stateFor(1);
		manager.sync([tab(1)], 1);
		expect(manager.stateFor(1)).toBe(before);
		expect(client.openCount).toBe(1);
	});

	it('releases the listing of a tab that closes, and one still opening', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1)], 1);
		await settle();
		manager.sync([], null);
		expect(client.openCount).toBe(0);

		manager.sync([tab(2)], 2);
		manager.sync([], null);
		await settle();
		expect(client.openCount).toBe(0);
	});

	it('reports a folder that cannot open as an error state, then recovers on navigation', async () => {
		const { client, manager } = setup();
		client.failOpening(B, { kind: 'permissionDenied', location: B });
		manager.sync([tab(1, B)], 1);
		await settle();
		expect(manager.stateFor(1)).toMatchObject({
			status: 'error',
			error: { kind: 'permissionDenied' },
		});
		manager.sync([tab(1, A)], 1);
		await settle();
		expect(manager.stateFor(1)?.status).toBe('ready');
	});

	it('keeps a background tab for a while, then drops its pages but keeps the last ones stale', async () => {
		vi.useFakeTimers();
		const { client, manager } = setup({ evictDelayMs: 1000 });
		manager.sync([tab(1), tab(2, B)], 1);
		await vi.advanceTimersByTimeAsync(0);
		const state = manager.stateFor(1);
		if (state?.status !== 'ready') throw new Error('not ready');
		const model = state.session.model;
		model.ensure(0, 10);
		await vi.advanceTimersByTimeAsync(0);
		model.ensure(400, 420);
		await vi.advanceTimersByTimeAsync(0);
		const cached = model.cachedCount;
		expect(cached).toBeGreaterThan(256);

		manager.sync([tab(1), tab(2, B)], 2);
		await vi.advanceTimersByTimeAsync(0);
		expect(client.openCount).toBe(2);
		await vi.advanceTimersByTimeAsync(999);
		expect(model.cachedCount).toBe(cached);
		await vi.advanceTimersByTimeAsync(2);
		expect(model.cachedCount).toBeLessThan(cached);
		expect(model.staleCount).toBe(model.cachedCount);
		// What it last showed still paints, so returning does not flash blank.
		expect(model.entryAt(410)).toBeDefined();
		expect(model.hasFresh(410)).toBe(false);

		// Returning refetches the stale page in place.
		manager.sync([tab(1), tab(2, B)], 1);
		model.ensure(400, 420);
		await vi.advanceTimersByTimeAsync(0);
		expect(model.hasFresh(410)).toBe(true);
	});

	it('cancels the eviction when the tab returns in time, and closes a listing left for another folder', async () => {
		vi.useFakeTimers();
		const { client, manager } = setup({ evictDelayMs: 1000 });
		manager.sync([tab(1), tab(2, B)], 1);
		await vi.advanceTimersByTimeAsync(0);
		const state = manager.stateFor(1);
		if (state?.status !== 'ready') throw new Error('not ready');
		state.session.model.ensure(0, 10);
		await vi.advanceTimersByTimeAsync(0);
		manager.sync([tab(1), tab(2, B)], 2);
		await vi.advanceTimersByTimeAsync(500);
		manager.sync([tab(1), tab(2, B)], 1);
		await vi.advanceTimersByTimeAsync(2000);
		expect(state.session.model.staleCount).toBe(0);

		// A location change made while the tab is hidden releases its listing.
		manager.sync([tab(1, B), tab(2, B)], 2);
		await vi.advanceTimersByTimeAsync(0);
		expect(manager.stateFor(1)).toBeUndefined();
		expect(client.openCount).toBe(1);
	});

	it('can be reused after dispose', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1)], 1);
		await settle();
		manager.dispose();
		expect(client.openCount).toBe(0);
		manager.sync([tab(1)], 1);
		await settle();
		expect(client.openCount).toBe(1);
	});
});
