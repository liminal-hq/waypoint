// Verifies tab hints: read from a view, applied once to a restored tab's first listing, reported on a timer and on blur
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import { ListingManager } from './listingManager';
import { flushHints, followHints, HINT_INTERVAL_MS, readHints } from './tabHints';
import type { ListingSession } from './useListingSession';

const FOLDER = fileLocation('/a');
const OTHER = fileLocation('/b');
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function tab(id: number, hints: TabSnapshot['hints'], location = FOLDER): TabSnapshot {
	return { id, location, back: [], forward: [], pinned: false, colour: null, group: null, hints };
}

function setup(mode: 'list' | 'grid' = 'list') {
	const client = new FakeVfsClient();
	client.setFolder(FOLDER, [
		makeEntry(1, 'alpha.txt'),
		makeEntry(2, 'beta.txt'),
		makeEntry(3, 'gamma.txt'),
	]);
	client.setFolder(OTHER, [makeEntry(1, 'other.txt')]);
	return { client, manager: new ListingManager(client, { viewMode: () => mode }) };
}

async function ready(manager: ListingManager, id: number): Promise<ListingSession> {
	await settle();
	const state = manager.stateFor(id);
	if (state?.status !== 'ready') throw new Error('the listing is not ready');
	return state.session;
}

afterEach(() => vi.useRealTimers());

describe('applying hints', () => {
	it('gives a restored tab its scroll offset and focuses the entry by name', async () => {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 120, focused: 'beta.txt' })], new Set([1]));
		const session = await ready(manager, 1);
		await settle();
		expect(session.view.scrollTop).toBe(120);
		expect(session.view.gridScrollTop).toBe(0);
		expect(session.view.pendingScroll).toBe(120);
		const focus = session.store.getState().focus;
		expect(focus === null ? null : session.model.entryAt(focus)?.name).toBe('beta.txt');
		expect(session.store.getState().touched).toBe(false);
	});

	it('uses the grid offset when the grid is the layout', async () => {
		const { manager } = setup('grid');
		manager.sync([tab(1, { scrollTop: 300, focused: null })], new Set([1]));
		const session = await ready(manager, 1);
		expect(session.view.gridScrollTop).toBe(300);
		expect(session.view.scrollTop).toBe(0);
	});

	it('ignores a name the listing does not have', async () => {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: 'gone.txt' })], new Set([1]));
		const session = await ready(manager, 1);
		await settle();
		expect(session.store.getState().focus).toBeNull();
	});

	it('applies once: a tab that navigates on does not take its old hints again', async () => {
		const { manager } = setup();
		const hints = { scrollTop: 50, focused: null };
		manager.sync([tab(1, hints)], new Set([1]));
		await ready(manager, 1);
		manager.sync([tab(1, hints, OTHER)], new Set([1]));
		const next = await ready(manager, 1);
		expect(next.view.scrollTop).toBe(0);
	});

	it('does not apply them to a tab that was never restored with any', async () => {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: null })], new Set([1]));
		const session = await ready(manager, 1);
		expect(session.view.scrollTop).toBe(0);
		expect(session.store.getState().focus).toBeNull();
	});
});

describe('reading hints', () => {
	it('reads the layout in use and the focused entry', async () => {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: null })], new Set([1]));
		const session = await ready(manager, 1);
		await session.model.readRange(0, 3);
		session.view.scrollTop = 40.4;
		session.view.gridScrollTop = 900;
		session.store.getState().moveTo(2, false);
		expect(readHints(session, 'list')).toEqual({ scrollTop: 40, focused: 'gamma.txt' });
		expect(readHints(session, 'grid').scrollTop).toBe(900);
	});
});

describe('reporting hints', () => {
	async function reporting() {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: null })], new Set([1]));
		const session = await ready(manager, 1);
		const api = { setTabHints: vi.fn(async () => {}) };
		vi.useFakeTimers();
		const stop = followHints(api, () => ({ tab: 1, session, mode: 'list' }));
		return { api, session, stop };
	}

	it('reports a change at most once per interval, and nothing when nothing changed', async () => {
		const { api, session, stop } = await reporting();
		await vi.advanceTimersByTimeAsync(HINT_INTERVAL_MS);
		expect(api.setTabHints).toHaveBeenCalledTimes(1);
		await vi.advanceTimersByTimeAsync(HINT_INTERVAL_MS * 3);
		expect(api.setTabHints).toHaveBeenCalledTimes(1);
		session.view.scrollTop = 10;
		session.view.scrollTop = 20;
		await vi.advanceTimersByTimeAsync(HINT_INTERVAL_MS);
		expect(api.setTabHints).toHaveBeenCalledTimes(2);
		expect(api.setTabHints).toHaveBeenLastCalledWith(1, { scrollTop: 20, focused: null });
		stop();
	});

	it('reports when the shell asks for a flush, until stopped', async () => {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: null })], 1);
		const session = await ready(manager, 1);
		const api = { setTabHints: vi.fn(async () => {}) };
		let flush: (() => void) | null = null;
		const unsubscribe = vi.fn();
		const stop = followHints(
			api,
			() => ({ tab: 1, session, mode: 'list' }),
			HINT_INTERVAL_MS,
			undefined,
			(callback) => {
				flush = callback;
				return unsubscribe;
			},
		);
		session.view.scrollTop = 42;
		flush!();
		expect(api.setTabHints).toHaveBeenLastCalledWith(1, { scrollTop: 42, focused: null });
		stop();
		expect(unsubscribe).toHaveBeenCalledTimes(1);
	});

	it('reports when the page is hidden or about to unload', async () => {
		const { api, session, stop } = await reporting();
		session.view.scrollTop = 5;
		window.dispatchEvent(new Event('beforeunload'));
		expect(api.setTabHints).toHaveBeenLastCalledWith(1, { scrollTop: 5, focused: null });
		session.view.scrollTop = 6;
		vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden');
		document.dispatchEvent(new Event('visibilitychange'));
		expect(api.setTabHints).toHaveBeenLastCalledWith(1, { scrollTop: 6, focused: null });
		vi.restoreAllMocks();
		stop();
	});

	it('reports at once when the window loses focus or is going away', async () => {
		const { api, session, stop } = await reporting();
		session.view.scrollTop = 77;
		window.dispatchEvent(new Event('blur'));
		expect(api.setTabHints).toHaveBeenLastCalledWith(1, { scrollTop: 77, focused: null });
		session.view.scrollTop = 88;
		window.dispatchEvent(new Event('pagehide'));
		expect(api.setTabHints).toHaveBeenLastCalledWith(1, { scrollTop: 88, focused: null });
		stop();
		session.view.scrollTop = 99;
		window.dispatchEvent(new Event('blur'));
		expect(api.setTabHints).toHaveBeenCalledTimes(2);
	});
});

describe('flushing hints', () => {
	it('sends the tab’s hints now and resolves only once the session has them', async () => {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: null })], new Set([1]));
		const session = await ready(manager, 1);
		let accept: () => void = () => {};
		const setTabHints = vi.fn(
			() =>
				new Promise<void>((resolve) => {
					accept = resolve;
				}),
		);
		const stop = followHints({ setTabHints }, () => ({ tab: 1, session, mode: 'list' }));
		session.view.scrollTop = 321;
		let settled = false;
		const flushed = flushHints(1).then(() => {
			settled = true;
		});
		expect(setTabHints).toHaveBeenCalledWith(1, { scrollTop: 321, focused: null });
		await settle();
		expect(settled).toBe(false);
		accept();
		await flushed;
		expect(settled).toBe(true);
		stop();
	});

	it('finds a tab that is not the active one, and sends even hints it already reported', async () => {
		const { manager } = setup();
		manager.sync(
			[tab(1, { scrollTop: 0, focused: null }), tab(2, { scrollTop: 0, focused: null }, OTHER)],
			new Set([1]),
		);
		const first = await ready(manager, 1);
		const api = { setTabHints: vi.fn(async () => {}) };
		const stop = followHints(
			api,
			() => ({ tab: 1, session: first, mode: 'list' }),
			HINT_INTERVAL_MS,
			(id) => (id === 1 ? { tab: 1, session: first, mode: 'list' } : null),
		);
		await flushHints(1);
		await flushHints(1);
		expect(api.setTabHints).toHaveBeenCalledTimes(2);
		await flushHints(2);
		expect(api.setTabHints).toHaveBeenCalledTimes(2);
		stop();
	});

	it('resolves when the session cannot take the hints, and when nothing is reporting', async () => {
		await expect(flushHints(9)).resolves.toBeUndefined();
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: null })], new Set([1]));
		const session = await ready(manager, 1);
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const stop = followHints(
			{ setTabHints: vi.fn(async () => Promise.reject(new Error('gone'))) },
			() => ({ tab: 1, session, mode: 'list' }),
		);
		await expect(flushHints(1)).resolves.toBeUndefined();
		expect(warn).toHaveBeenCalled();
		stop();
		await flushHints(1);
	});
});

describe('a tab handed to a window that is running', () => {
	const arriving = (hints: TabSnapshot['hints']) => tab(7, hints);

	it('opens its listing in the target, applies the hints, and the source lets its own listing go', async () => {
		const client = new FakeVfsClient();
		client.setFolder(FOLDER, [
			makeEntry(1, 'alpha.txt'),
			makeEntry(2, 'beta.txt'),
			makeEntry(3, 'gamma.txt'),
		]);
		client.setFolder(OTHER, [makeEntry(1, 'other.txt')]);
		const source = new ListingManager(client, { viewMode: () => 'list' });
		const target = new ListingManager(client, { viewMode: () => 'list' });
		source.sync(
			[tab(7, { scrollTop: 0, focused: null }), tab(8, { scrollTop: 0, focused: null }, OTHER)],
			new Set([7]),
		);
		target.sync([tab(9, { scrollTop: 0, focused: null }, OTHER)], new Set([9]));
		await ready(source, 7);
		await settle();
		expect(client.openCount).toBe(2);

		// The hand-off: the target gains the tab (with the hints the source just reported), the
		// source loses it.
		const moved = arriving({ scrollTop: 240, focused: 'beta.txt' });
		target.sync([tab(9, { scrollTop: 0, focused: null }, OTHER), moved], new Set([7]));
		source.sync([tab(8, { scrollTop: 0, focused: null }, OTHER)], new Set([8]));
		const session = await ready(target, 7);
		await settle();

		expect(source.stateFor(7)).toBeUndefined();
		expect(target.openCount).toBe(2);
		// The source's listing for tab 7 closed and the target's opened; tab 8 opened in the source.
		expect(source.openCount).toBe(1);
		expect(client.openCount).toBe(3);
		expect(session.view.scrollTop).toBe(240);
		expect(session.view.pendingScroll).toBe(240);
		const focus = session.store.getState().focus;
		expect(focus === null ? null : session.model.entryAt(focus)?.name).toBe('beta.txt');
	});

	it('keeps the hints for a tab that arrives in the background until its listing opens', async () => {
		const { manager } = setup();
		manager.sync([tab(9, { scrollTop: 0, focused: null }, OTHER)], new Set([9]));
		await ready(manager, 9);
		manager.sync(
			[tab(9, { scrollTop: 0, focused: null }, OTHER), arriving({ scrollTop: 90, focused: null })],
			new Set([9]),
		);
		expect(manager.stateFor(7)).toBeUndefined();
		manager.sync(
			[tab(9, { scrollTop: 0, focused: null }, OTHER), arriving({ scrollTop: 90, focused: null })],
			new Set([7]),
		);
		const session = await ready(manager, 7);
		expect(session.view.scrollTop).toBe(90);
	});

	it('applies the hints again when a tab leaves and comes back', async () => {
		const { manager } = setup();
		const hints = { scrollTop: 60, focused: null };
		manager.sync([tab(7, hints)], new Set([7]));
		await ready(manager, 7);
		manager.sync([], new Set());
		manager.sync([tab(7, { scrollTop: 130, focused: null })], new Set([7]));
		const session = await ready(manager, 7);
		expect(session.view.scrollTop).toBe(130);
	});
});
