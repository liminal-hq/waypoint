// Tests for the real VfsClient with the plugin's guest-js module mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import type { ListingSnapshot } from '@liminal-hq/waypoint-protocol/generated/ListingSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriVfsClient } from './tauriVfsClient';
import { isVfsError } from './vfsClient';

const plugin = vi.hoisted(() => ({
	openListing: vi.fn(),
	getRange: vi.fn(),
	setSort: vi.fn(),
	setFilter: vi.fn(),
	closeListing: vi.fn(),
	onListingEvent: vi.fn(),
}));
vi.mock('@liminal-hq/waypoint-plugin-vfs', () => plugin);

const home: Location = { display: '/home/a', uri: 'file:///home/a' };
const snapshot: ListingSnapshot = {
	handle: 1,
	location: home,
	revision: 1,
	count: 0,
	phase: 'scanning',
	sort: { key: 'name', descending: false, directoriesFirst: true },
	filter: { showHidden: false },
};

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

describe('createTauriVfsClient', () => {
	it('passes every call through to the plugin and returns its result', async () => {
		const client = createTauriVfsClient();
		const sort: SortSpec = { key: 'size', descending: true, directoriesFirst: false };
		plugin.openListing.mockResolvedValue(snapshot);
		plugin.getRange.mockResolvedValue([]);
		plugin.setSort.mockResolvedValue({ ...snapshot, revision: 2 });
		plugin.setFilter.mockResolvedValue({ ...snapshot, revision: 3 });
		plugin.closeListing.mockResolvedValue(undefined);

		expect(await client.openListing(home, { sort })).toBe(snapshot);
		expect(plugin.openListing).toHaveBeenCalledWith(home, { sort });
		expect(await client.getRange(1, 10, 20)).toEqual([]);
		expect(plugin.getRange).toHaveBeenCalledWith(1, 10, 20);
		expect((await client.setSort(1, sort)).revision).toBe(2);
		expect(plugin.setSort).toHaveBeenCalledWith(1, sort);
		expect((await client.setFilter(1, { showHidden: true })).revision).toBe(3);
		expect(plugin.setFilter).toHaveBeenCalledWith(1, { showHidden: true });
		await client.closeListing(1);
		expect(plugin.closeListing).toHaveBeenCalledWith(1);
	});

	it('rejects with the plugin VfsError untouched', async () => {
		const error = { kind: 'notFound', location: home };
		plugin.openListing.mockRejectedValue(error);
		plugin.getRange.mockRejectedValue({ kind: 'staleHandle' });
		const client = createTauriVfsClient();
		await expect(client.openListing(home)).rejects.toBe(error);
		const stale = await client.getRange(9, 0, 1).catch((e: unknown) => e);
		expect(isVfsError(stale) && stale.kind).toBe('staleHandle');
	});

	it('turns a stray string or Error rejection into an io VfsError', async () => {
		const client = createTauriVfsClient();
		plugin.setSort.mockRejectedValue('command not allowed');
		plugin.setFilter.mockRejectedValue(new Error('bridge is gone'));
		const denied = await client.setSort(1, snapshot.sort).catch((e: unknown) => e);
		expect(denied).toEqual({ kind: 'io', message: 'command not allowed', location: null });
		const gone = await client.setFilter(1, { showHidden: false }).catch((e: unknown) => e);
		expect(gone).toEqual({ kind: 'io', message: 'bridge is gone', location: null });
	});

	it('delivers events to the listener and stops after unsubscribing', async () => {
		let deliver: (event: ListingEvent) => void = () => {};
		const unlisten = vi.fn();
		plugin.onListingEvent.mockImplementation((handler: (event: ListingEvent) => void) => {
			deliver = handler;
			return Promise.resolve(unlisten);
		});
		const client = createTauriVfsClient();
		const seen: ListingEvent[] = [];
		const stop = client.onListingEvent((event) => seen.push(event));
		await Promise.resolve();

		const progress: ListingEvent = {
			kind: 'progress',
			handle: 1,
			revision: 2,
			phase: 'ready',
			scanned: 3,
			count: 3,
		};
		deliver(progress);
		expect(seen).toEqual([progress]);

		stop();
		expect(unlisten).toHaveBeenCalledTimes(1);
		deliver(progress);
		expect(seen).toHaveLength(1);
		stop();
		expect(unlisten).toHaveBeenCalledTimes(1);
	});

	it('undoes a subscription that is cancelled before it was registered', async () => {
		let register: (stop: () => void) => void = () => {};
		const unlisten = vi.fn();
		plugin.onListingEvent.mockReturnValue(
			new Promise<() => void>((resolve) => {
				register = resolve;
			}),
		);
		const stop = createTauriVfsClient().onListingEvent(() => {});
		stop();
		register(unlisten);
		await Promise.resolve();
		expect(unlisten).toHaveBeenCalledTimes(1);
	});
});
