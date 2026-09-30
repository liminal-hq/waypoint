// Shared helpers for the file-view tests: a seeded random source, a folder fixture and a layout stub
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { FakeVfsClient, fileLocation, syntheticEntries } from '../services/fakeVfsClient';

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
 * rows. This gives every element a height (and the row height through `getComputedStyle`).
 */
export function stubLayout(viewportHeight: number): () => void {
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
	return () => {
		if (descriptor) Object.defineProperty(HTMLElement.prototype, 'offsetHeight', descriptor);
		if (client) Object.defineProperty(HTMLElement.prototype, 'clientHeight', client);
	};
}
