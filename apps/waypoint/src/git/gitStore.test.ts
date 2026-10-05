// Verifies the window's Git store: one watch per folder however many ask, events by revision, and letting go
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { cleanSummary, createFakeGitClient } from '../services/fakeGitClient';
import { fileLocation } from '../services/fakeVfsClient';
import { GitStore } from './gitStore';

const ROOT = fileLocation('/home/me/repo');
const INSIDE = fileLocation('/home/me/repo/src');
const OUTSIDE = fileLocation('/home/me/elsewhere');

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

function setup() {
	const client = createFakeGitClient([{ root: ROOT, name: 'repo', summary: cleanSummary() }]);
	const store = new GitStore(client);
	return { client, store };
}

describe('watching a folder', () => {
	it('finds the repository a folder is in and says nothing for a folder that is in none', async () => {
		const { store } = setup();
		store.acquire(INSIDE);
		store.acquire(OUTSIDE);
		await settle();
		expect(store.repository(INSIDE)?.name).toBe('repo');
		expect(store.repository(INSIDE)?.root).toEqual(ROOT);
		expect(store.repository(INSIDE)?.summary?.head).toBe('main');
		expect(store.repository(OUTSIDE)).toBeNull();
		expect(store.known(OUTSIDE)).toBe(true);
		expect(store.known(fileLocation('/never/asked'))).toBe(false);
	});

	it('watches a folder once however many parts of the window ask, and lets go with the last', async () => {
		const { client, store } = setup();
		const first = store.acquire(INSIDE);
		const second = store.acquire(INSIDE);
		await settle();
		expect(client.watched).toHaveLength(1);
		expect(client.watching.size).toBe(1);
		first();
		first();
		expect(client.watching.size).toBe(1);
		second();
		await settle();
		expect(client.watching.size).toBe(0);
		expect(store.watching).toBe(0);
	});

	it('lets go of a watch that answers after nobody wants it any more', async () => {
		const { client, store } = setup();
		const hold = client.holdWatch();
		const release = store.acquire(INSIDE);
		release();
		hold.release();
		await settle();
		expect(client.watching.size).toBe(0);
	});
});

describe('hearing changes', () => {
	it('replaces the summary when the plugin reports a newer one', async () => {
		const { client, store } = setup();
		store.acquire(INSIDE);
		await settle();
		const listener = vi.fn();
		store.subscribe(listener);
		const before = store.getVersion();
		client.change(ROOT.uri, cleanSummary({ untracked: 2, head: 'feature' }));
		expect(store.repository(INSIDE)?.summary).toMatchObject({ head: 'feature', untracked: 2 });
		expect(store.repository(INSIDE)?.revision).toBe(2);
		expect(listener).toHaveBeenCalled();
		expect(store.getVersion()).toBeGreaterThan(before);
	});

	it('ignores an event for a revision it already has, and one for a watch it does not hold', async () => {
		const { client, store } = setup();
		store.acquire(INSIDE);
		await settle();
		const watch = store.repository(INSIDE)!;
		client.change(ROOT.uri, cleanSummary({ head: 'newer' }));
		const listener = vi.fn();
		store.subscribe(listener);
		// A late copy of the first revision, and a repeat of the second.
		client.emit({ id: watch.id, revision: 1, summary: cleanSummary({ head: 'stale' }) });
		client.emit({ id: watch.id, revision: 2, summary: cleanSummary({ head: 'repeat' }) });
		client.emit({ id: watch.id + 99, revision: 9, summary: cleanSummary({ head: 'nobody' }) });
		expect(listener).not.toHaveBeenCalled();
		expect(store.repository(INSIDE)?.summary?.head).toBe('newer');
	});

	it('hands out the same repository object until something changes, for useSyncExternalStore', async () => {
		const { client, store } = setup();
		store.acquire(INSIDE);
		await settle();
		const once = store.repository(INSIDE);
		expect(store.repository(INSIDE)).toBe(once);
		client.change(ROOT.uri, cleanSummary({ staged: 1 }));
		expect(store.repository(INSIDE)).not.toBe(once);
	});
});

describe('disposing', () => {
	it('lets go of every watch and ignores what comes after', async () => {
		const { client, store } = setup();
		store.acquire(INSIDE);
		store.acquire(ROOT);
		await settle();
		expect(client.watching.size).toBe(2);
		store.dispose();
		await settle();
		expect(client.watching.size).toBe(0);
		expect(store.acquire(INSIDE)).toBeTypeOf('function');
		expect(store.watching).toBe(0);
	});

	it('survives a plugin that rejects', async () => {
		const client = createFakeGitClient();
		client.watch = () => Promise.reject(new Error('no plugin'));
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const store = new GitStore(client);
		store.acquire(INSIDE);
		await settle();
		expect(store.repository(INSIDE)).toBeNull();
		expect(store.known(INSIDE)).toBe(true);
		warn.mockRestore();
	});
});
