// Verifies the window's copy of the remembered folder views: revision-gated events, missed changes and the writes in flight
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeFolderViewsClient, MAX_FOLDERS } from '../services/fakeFolderViewsClient';
import { createFolderViewsStore } from './folderViewStore';

const GRID = { mode: 'grid', sort: null } as const;
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('the folder views store', () => {
	it('starts empty and not ready, then holds what Rust says is remembered', async () => {
		const client = createFakeFolderViewsClient({ 'file:///a': GRID });
		const handle = createFolderViewsStore(client);
		expect(handle.store.getState()).toMatchObject({ ready: false, revision: 0, writing: 0 });
		expect(handle.store.getState().folders.size).toBe(0);
		await handle.ready;
		expect(handle.store.getState().ready).toBe(true);
		expect(handle.store.getState().folders.get('file:///a')).toEqual(GRID);
		handle.dispose();
	});

	it('follows changes in order, adding, changing and forgetting a folder', async () => {
		const client = createFakeFolderViewsClient();
		const handle = createFolderViewsStore(client);
		await handle.ready;
		client.change('file:///a', GRID);
		client.change('file:///a', { mode: null, sort: { ...sortOf(), key: 'size' } });
		expect(handle.store.getState().revision).toBe(2);
		expect(handle.store.getState().folders.get('file:///a')).toMatchObject({
			mode: 'grid',
			sort: { key: 'size' },
		});
		await client.reset('file:///a');
		expect(handle.store.getState().folders.has('file:///a')).toBe(false);
		expect(handle.store.getState().revision).toBe(3);
	});

	it('keeps the events that arrive before the snapshot and applies those it has not seen', async () => {
		const client = createFakeFolderViewsClient();
		const slow = client.holdSnapshot();
		const handle = createFolderViewsStore(client);
		client.change('file:///a', GRID);
		client.change('file:///b', GRID);
		expect(handle.store.getState().ready).toBe(false);
		slow.release();
		await handle.ready;
		expect([...handle.store.getState().folders.keys()]).toEqual(['file:///a', 'file:///b']);
		expect(handle.store.getState().revision).toBe(2);
	});

	it('ignores an event it already has and a repeated one', async () => {
		const client = createFakeFolderViewsClient();
		const handle = createFolderViewsStore(client);
		await handle.ready;
		client.change('file:///a', GRID);
		const before = handle.store.getState().folders;
		client.emit({ revision: 1, changes: [{ key: 'file:///a', view: null }] });
		client.emit({ revision: 0, changes: [{ key: 'file:///b', view: GRID }] });
		expect(handle.store.getState().folders).toBe(before);
	});

	it('reads the snapshot again when an event shows that one was missed', async () => {
		const client = createFakeFolderViewsClient();
		const handle = createFolderViewsStore(client);
		await handle.ready;
		client.change('file:///a', GRID);
		client.change('file:///b', GRID);
		client.change('file:///c', GRID);
		// An event whose revision skips ahead means changes were missed: the snapshot is read again.
		client.emit({ revision: 9, changes: [{ key: 'file:///z', view: GRID }] });
		await settle();
		expect([...handle.store.getState().folders.keys()].sort()).toEqual([
			'file:///a',
			'file:///b',
			'file:///c',
		]);
		expect(handle.store.getState().revision).toBe(3);
	});

	it('counts a write as in flight until Rust answers, and passes only what was chosen', async () => {
		const client = createFakeFolderViewsClient();
		const handle = createFolderViewsStore(client);
		await handle.ready;
		const held = client.holdWrites();
		const first = handle.remember('file:///a', { mode: 'grid' });
		const second = handle.reset('file:///b');
		expect(handle.store.getState().writing).toBe(2);
		expect(client.remembered).toEqual([{ key: 'file:///a', patch: { mode: 'grid', sort: null } }]);
		held.release();
		await Promise.all([first, second]);
		expect(handle.store.getState().writing).toBe(0);
	});

	it('rejects with Rust’s refusal, and counts nothing as in flight afterwards', async () => {
		const client = createFakeFolderViewsClient();
		const handle = createFolderViewsStore(client);
		await handle.ready;
		client.failNext();
		await expect(handle.remember('file:///a', { mode: 'grid' })).rejects.toMatchObject({
			kind: 'storage',
		});
		expect(handle.store.getState().writing).toBe(0);
		expect(handle.store.getState().folders.size).toBe(0);
	});

	it('forgets the folders written longest ago when Rust prunes them', async () => {
		const client = createFakeFolderViewsClient();
		const handle = createFolderViewsStore(client);
		await handle.ready;
		for (let i = 0; i <= MAX_FOLDERS; i += 1) client.change(`file:///${i}`, GRID);
		expect(handle.store.getState().folders.size).toBe(MAX_FOLDERS);
		expect(handle.store.getState().folders.has('file:///0')).toBe(false);
		expect(handle.store.getState().folders.has(`file:///${MAX_FOLDERS}`)).toBe(true);
	});

	it('stops following once disposed, and keeps no folder when the snapshot cannot be read', async () => {
		const client = createFakeFolderViewsClient();
		const handle = createFolderViewsStore(client);
		await handle.ready;
		handle.dispose();
		client.change('file:///a', GRID);
		expect(handle.store.getState().folders.size).toBe(0);

		const broken = createFakeFolderViewsClient();
		broken.snapshot = () => Promise.reject(new Error('no plugin'));
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const none = createFolderViewsStore(broken);
		await none.ready;
		expect(none.store.getState()).toMatchObject({ ready: false });
		expect(none.store.getState().folders.size).toBe(0);
		warn.mockRestore();
	});
});

function sortOf() {
	return { key: 'name', descending: false, directoriesFirst: true, groupBy: 'none' } as const;
}
