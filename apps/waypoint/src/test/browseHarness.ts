// Shared helpers for the file-view tests: a seeded random source, a folder fixture and a layout stub
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingSnapshot } from '@liminal-hq/waypoint-protocol/generated/ListingSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VfsClient } from '../services/vfsClient';
import {
	FakeVfsClient,
	fileLocation,
	makeEntry,
	syntheticEntries,
} from '../services/fakeVfsClient';

export const FOLDER: Location = fileLocation('/home/test');

/** A small deterministic random generator (mulberry32), so a failing sequence can be replayed from its seed. */
export function seededRandom(seed: number): () => number {
	let state = seed >>> 0;
	return () => {
		state = (state + 0x6d2b79f5) >>> 0;
		let t = state;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

/** A fake client serving `count` synthetic entries at `FOLDER`. */
export function clientWith(
	count: number,
	latencyMs = 0,
): { client: FakeVfsClient; entries: Entry[] } {
	const client = new FakeVfsClient(latencyMs ? { latencyMs } : {});
	const entries = syntheticEntries(count);
	client.setFolder(FOLDER, entries);
	return { client, entries };
}

/**
 * happy-dom does no layout, so the virtualiser would see a zero-height viewport and render no
 * rows. This gives every element a height (and the row height through `getComputedStyle`), and a
 * width when one is given, for the grid's column count.
 */
export function stubLayout(viewportHeight: number, viewportWidth = 0): () => void {
	const descriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
	const client = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientHeight');
	Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
		configurable: true,
		get: () => viewportHeight,
	});
	Object.defineProperty(HTMLElement.prototype, 'clientHeight', {
		configurable: true,
		get: () => viewportHeight,
	});
	const width = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth');
	if (viewportWidth > 0) {
		Object.defineProperty(HTMLElement.prototype, 'clientWidth', {
			configurable: true,
			get: () => viewportWidth,
		});
	}
	return () => {
		if (width) Object.defineProperty(HTMLElement.prototype, 'clientWidth', width);
		else if (viewportWidth > 0) Reflect.deleteProperty(HTMLElement.prototype, 'clientWidth');
		if (descriptor) Object.defineProperty(HTMLElement.prototype, 'offsetHeight', descriptor);
		if (client) Object.defineProperty(HTMLElement.prototype, 'clientHeight', client);
	};
}

/**
 * A client that answers with `overrides` and falls back to `base` for everything else, so a test
 * stubs one or two methods without restating the whole interface.
 */
export function withOverrides(base: VfsClient, overrides: Partial<VfsClient>): VfsClient {
	return Object.assign(Object.create(base) as VfsClient, overrides);
}

/** A client that reports a huge folder without holding it, to test the scroll cap. */
export function hugeClient(count: number): VfsClient {
	const snapshot: ListingSnapshot = {
		handle: 1,
		location: FOLDER,
		revision: 1,
		count,
		phase: 'ready',
		sort: { key: 'name', descending: false, directoriesFirst: true },
		filter: { showHidden: false },
	};
	return withOverrides(new FakeVfsClient(), {
		openListing: async () => snapshot,
		getRange: async (_handle, start, length) =>
			Array.from({ length: Math.min(length, count - start) }, (_, i) =>
				makeEntry(start + i, `entry-${start + i}.txt`),
			),
		setSort: async () => snapshot,
		setFilter: async () => snapshot,
		closeListing: async () => {},
		onListingEvent: () => () => {},
	});
}
