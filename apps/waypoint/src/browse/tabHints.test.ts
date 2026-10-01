// Verifies tab hints: read from a view, applied once to a restored tab's first listing, reported on a timer and on blur
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import { ListingManager } from './listingManager';
import { followHints, HINT_INTERVAL_MS, readHints } from './tabHints';
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
		manager.sync([tab(1, { scrollTop: 120, focused: 'beta.txt' })], 1);
		const session = await ready(manager, 1);
		await settle();
		expect(session.view.scrollTop).toBe(120);
		expect(session.view.gridScrollTop).toBe(0);
		const focus = session.store.getState().focus;
		expect(focus === null ? null : session.model.entryAt(focus)?.name).toBe('beta.txt');
		expect(session.store.getState().touched).toBe(false);
	});

	it('uses the grid offset when the grid is the layout', async () => {
		const { manager } = setup('grid');
		manager.sync([tab(1, { scrollTop: 300, focused: null })], 1);
		const session = await ready(manager, 1);
		expect(session.view.gridScrollTop).toBe(300);
		expect(session.view.scrollTop).toBe(0);
	});

	it('ignores a name the listing does not have', async () => {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: 'gone.txt' })], 1);
		const session = await ready(manager, 1);
		await settle();
		expect(session.store.getState().focus).toBeNull();
	});

	it('applies once: a tab that navigates on does not take its old hints again', async () => {
		const { manager } = setup();
		const hints = { scrollTop: 50, focused: null };
		manager.sync([tab(1, hints)], 1);
		await ready(manager, 1);
		manager.sync([tab(1, hints, OTHER)], 1);
		const next = await ready(manager, 1);
		expect(next.view.scrollTop).toBe(0);
	});

	it('does not apply them to a tab that was never restored with any', async () => {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: null })], 1);
		const session = await ready(manager, 1);
		expect(session.view.scrollTop).toBe(0);
		expect(session.store.getState().focus).toBeNull();
	});
});

describe('reading hints', () => {
	it('reads the layout in use and the focused entry', async () => {
		const { manager } = setup();
		manager.sync([tab(1, { scrollTop: 0, focused: null })], 1);
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
		manager.sync([tab(1, { scrollTop: 0, focused: null })], 1);
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
