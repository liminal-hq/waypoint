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
	return {
		id,
		location,
		back: [],
		forward: [],
		pinned: false,
		colour: null,
		group: null,
		hints: { scrollTop: 0, focused: null },
	};
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
	it('opens again only the failed listings on screen that the caller retries', async () => {
		const { client, manager } = setup();
		const offline = { kind: 'disconnected' as const, location: A };
		client.failOpening(A, offline);
		client.failOpening(B, { kind: 'notFound', location: B });
		manager.sync([tab(1), tab(2, B), tab(3, A)], new Set([1, 2]));
		await settle();
		expect(manager.stateFor(1)).toEqual({ status: 'error', error: offline });
		client.succeedOpening(A);
		client.succeedOpening(B);
		const retried = manager.retryFailed((error) => error.kind === 'disconnected');
		expect(retried).toBe(1);
		await settle();
		expect(manager.stateFor(1)?.status).toBe('ready');
		expect(manager.stateFor(2)?.status).toBe('error');
		expect(manager.stateFor(3)).toBeUndefined();
	});

	it('opens the active tab only, and shows opening before ready', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1), tab(2, B)], new Set([1]));
		expect(manager.stateFor(1)).toEqual({ status: 'opening' });
		expect(manager.stateFor(2)).toBeUndefined();
		await settle();
		expect(manager.stateFor(1)?.status).toBe('ready');
		expect(client.openCount).toBe(1);
	});

	it('holds no listing for Overview, which is a page, and lets go of the folder a tab moves off', async () => {
		const { client, manager } = setup();
		const overview = { display: 'Overview', uri: 'overview:/' };
		manager.sync([tab(1, overview)], new Set([1]));
		await settle();
		expect(manager.stateFor(1)).toBeUndefined();
		expect(client.openCount).toBe(0);
		manager.sync([tab(1, A)], new Set([1]));
		await settle();
		expect(manager.stateFor(1)?.status).toBe('ready');
		manager.sync([tab(1, overview)], new Set([1]));
		expect(manager.stateFor(1)).toBeUndefined();
		expect(manager.openCount).toBe(0);
	});

	it('opens a live listing for every visible tab, and evicts one that leaves the screen', async () => {
		vi.useFakeTimers();
		const { client, manager } = setup({ evictDelayMs: 1000 });
		manager.sync([tab(1), tab(2, B), tab(3)], new Set([1, 2]));
		await vi.advanceTimersByTimeAsync(0);
		expect(manager.stateFor(1)?.status).toBe('ready');
		expect(manager.stateFor(2)?.status).toBe('ready');
		expect(manager.stateFor(3)).toBeUndefined();
		expect(client.openCount).toBe(2);

		// Both panes stay live however long they sit there; a pane that is hidden again evicts.
		await vi.advanceTimersByTimeAsync(5000);
		const pane = manager.stateFor(2);
		if (pane?.status !== 'ready') throw new Error('not ready');
		pane.session.model.ensure(0, 10);
		await vi.advanceTimersByTimeAsync(0);
		const cached = pane.session.model.cachedCount;
		expect(cached).toBeGreaterThan(0);
		manager.sync([tab(1), tab(2, B), tab(3)], new Set([1]));
		expect(client.openCount).toBe(2);
		await vi.advanceTimersByTimeAsync(1100);
		expect(pane.session.model.staleCount).toBe(pane.session.model.cachedCount);
	});

	it('closes the old listing when the tab navigates and inherits its sort', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1)], new Set([1]));
		await settle();
		const first = manager.stateFor(1);
		if (first?.status !== 'ready') throw new Error('not ready');
		await first.session.model.setSort({
			key: 'size',
			descending: true,
			directoriesFirst: true,
			groupBy: 'none',
		});

		manager.sync([tab(1, B)], new Set([1]));
		await settle();
		expect(client.openCount).toBe(1);
		const next = manager.stateFor(1);
		expect(next?.status === 'ready' && next.session.model.sort.key).toBe('size');
	});

	it('reports the change of sort or grouping of a folder listing, and not a change of filter', async () => {
		const onSort = vi.fn();
		const { manager } = setup({ onSort });
		manager.sync([tab(1)], new Set([1]));
		await settle();
		const state = manager.stateFor(1);
		if (state?.status !== 'ready') throw new Error('not ready');
		const { model } = state.session;
		await model.setFilter({ showHidden: true });
		expect(onSort).not.toHaveBeenCalled();
		const grouped = { ...model.sort, groupBy: 'kind' } as const;
		await model.setSort(grouped);
		expect(onSort).toHaveBeenCalledTimes(1);
		expect(onSort).toHaveBeenLastCalledWith(grouped, A);
		await model.setSort({ ...grouped });
		expect(onSort).toHaveBeenCalledTimes(1);
	});

	it('does not reopen for a new object at the same location', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1)], new Set([1]));
		await settle();
		const before = manager.stateFor(1);
		manager.sync([tab(1)], new Set([1]));
		expect(manager.stateFor(1)).toBe(before);
		expect(client.openCount).toBe(1);
	});

	it('releases the listing of a tab that closes, and one still opening', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1)], new Set([1]));
		await settle();
		manager.sync([], new Set());
		expect(client.openCount).toBe(0);

		manager.sync([tab(2)], new Set([2]));
		manager.sync([], new Set());
		await settle();
		expect(client.openCount).toBe(0);
	});

	it('reports a folder that cannot open as an error state, then recovers on navigation', async () => {
		const { client, manager } = setup();
		client.failOpening(B, { kind: 'permissionDenied', location: B });
		manager.sync([tab(1, B)], new Set([1]));
		await settle();
		expect(manager.stateFor(1)).toMatchObject({
			status: 'error',
			error: { kind: 'permissionDenied' },
		});
		manager.sync([tab(1, A)], new Set([1]));
		await settle();
		expect(manager.stateFor(1)?.status).toBe('ready');
	});

	it('keeps a background tab for a while, then drops its pages but keeps the last ones stale', async () => {
		vi.useFakeTimers();
		const { client, manager } = setup({ evictDelayMs: 1000 });
		manager.sync([tab(1), tab(2, B)], new Set([1]));
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

		manager.sync([tab(1), tab(2, B)], new Set([2]));
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
		manager.sync([tab(1), tab(2, B)], new Set([1]));
		model.ensure(400, 420);
		await vi.advanceTimersByTimeAsync(0);
		expect(model.hasFresh(410)).toBe(true);
	});

	it('schedules eviction when a listing finishes opening after its tab went to the background', async () => {
		vi.useFakeTimers();
		const { manager } = setup({ evictDelayMs: 1000 });
		manager.sync([tab(1), tab(2, B)], new Set([1]));
		manager.sync([tab(1), tab(2, B)], new Set([2]));
		await vi.advanceTimersByTimeAsync(0);
		const state = manager.stateFor(1);
		if (state?.status !== 'ready') throw new Error('not ready');
		state.session.model.ensure(0, 10);
		await vi.advanceTimersByTimeAsync(0);
		await vi.advanceTimersByTimeAsync(1100);
		expect(state.session.model.staleCount).toBeGreaterThan(0);
	});

	it('cancels the eviction when the tab returns in time, and closes a listing left for another folder', async () => {
		vi.useFakeTimers();
		const { client, manager } = setup({ evictDelayMs: 1000 });
		manager.sync([tab(1), tab(2, B)], new Set([1]));
		await vi.advanceTimersByTimeAsync(0);
		const state = manager.stateFor(1);
		if (state?.status !== 'ready') throw new Error('not ready');
		state.session.model.ensure(0, 10);
		await vi.advanceTimersByTimeAsync(0);
		manager.sync([tab(1), tab(2, B)], new Set([2]));
		await vi.advanceTimersByTimeAsync(500);
		manager.sync([tab(1), tab(2, B)], new Set([1]));
		await vi.advanceTimersByTimeAsync(2000);
		expect(state.session.model.staleCount).toBe(0);

		// A location change made while the tab is hidden releases its listing.
		manager.sync([tab(1, B), tab(2, B)], new Set([2]));
		await vi.advanceTimersByTimeAsync(0);
		expect(manager.stateFor(1)).toBeUndefined();
		expect(client.openCount).toBe(1);
	});

	it('can be reused after dispose', async () => {
		const { client, manager } = setup();
		manager.sync([tab(1)], new Set([1]));
		await settle();
		manager.dispose();
		expect(client.openCount).toBe(0);
		manager.sync([tab(1)], new Set([1]));
		await settle();
		expect(client.openCount).toBe(1);
	});

	it('lets the last hidden-files choice win when toggled twice quickly', async () => {
		const { manager } = setup();
		manager.sync([tab(1)], new Set([1]));
		await settle();
		manager.setShowHidden(true);
		manager.setShowHidden(false);
		await settle();
		const state = manager.stateFor(1);
		expect(state?.status === 'ready' && state.session.model.filter.showHidden).toBe(false);
	});

	it('applies a hidden-files choice made while a listing was still opening', async () => {
		const { manager } = setup();
		manager.sync([tab(1)], new Set([1]));
		manager.setShowHidden(true);
		await settle();
		await settle();
		const state = manager.stateFor(1);
		expect(state?.status === 'ready' && state.session.model.filter.showHidden).toBe(true);
	});

	describe('retain', () => {
		it('keeps a listing open after its tab navigates, and closes it a moment after the hold ends', async () => {
			vi.useFakeTimers();
			const { client, manager } = setup({ retainGraceMs: 500 });
			manager.sync([tab(1)], new Set([1]));
			await vi.advanceTimersByTimeAsync(0);
			const release = manager.retain(1);
			expect(release).not.toBeNull();
			manager.sync([tab(1, B)], new Set([1]));
			await vi.advanceTimersByTimeAsync(0);
			expect(client.openCount).toBe(2);
			release?.();
			// Still there for a tab that is on its way back.
			await vi.advanceTimersByTimeAsync(499);
			expect(client.openCount).toBe(2);
			await vi.advanceTimersByTimeAsync(1);
			expect(client.openCount).toBe(1);
		});

		it('gives the session back to a tab that returns just after the hold ended', async () => {
			vi.useFakeTimers();
			const { client, manager } = setup({ retainGraceMs: 500 });
			manager.sync([tab(1)], new Set([1]));
			await vi.advanceTimersByTimeAsync(0);
			const first = manager.stateFor(1);
			const release = manager.retain(1);
			manager.sync([tab(1, B)], new Set([1]));
			await vi.advanceTimersByTimeAsync(0);
			release?.();
			manager.sync([tab(1)], new Set([1]));
			const back = manager.stateFor(1);
			expect(back?.status === 'ready' && first?.status === 'ready' && back.session).toBe(
				first?.status === 'ready' ? first.session : null,
			);
			// Adopted, so the grace period closes nothing.
			await vi.advanceTimersByTimeAsync(1000);
			expect(client.openCount).toBe(1);
		});

		it('closes what it kept when the manager is disposed', async () => {
			const { client, manager } = setup();
			manager.sync([tab(1)], new Set([1]));
			await settle();
			manager.retain(1);
			manager.sync([tab(1, B)], new Set([1]));
			await settle();
			manager.dispose();
			expect(client.openCount).toBe(0);
		});

		it('gives the tab the same session back when it returns to the folder during the hold', async () => {
			const { client, manager } = setup();
			manager.sync([tab(1)], new Set([1]));
			await settle();
			const first = manager.stateFor(1);
			const release = manager.retain(1);
			manager.sync([tab(1, B)], new Set([1]));
			await settle();
			manager.sync([tab(1)], new Set([1]));
			const back = manager.stateFor(1);
			expect(back?.status === 'ready' && first?.status === 'ready' && back.session).toBe(
				first?.status === 'ready' ? first.session : null,
			);
			// Only the one listing is open (the folder it left was closed), and it stays open after the hold ends.
			expect(client.openCount).toBe(1);
			release?.();
			expect(client.openCount).toBe(1);
		});

		it('has nothing to hold for a tab with no ready listing', () => {
			const { manager } = setup();
			manager.sync([tab(1)], new Set([1]));
			expect(manager.retain(1)).toBeNull();
			expect(manager.retain(9)).toBeNull();
		});
	});
});
