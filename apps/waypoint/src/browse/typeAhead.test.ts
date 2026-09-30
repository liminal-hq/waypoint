// Verifies type-ahead: the prefix buffer's pause, and the search through pages the view never loaded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { FOLDER } from '../test/browseHarness';
import { openListingModel } from './listingModel';
import { findByPrefix, TYPE_AHEAD_RESET_MS, TypeAheadBuffer } from './typeAhead';

describe('TypeAheadBuffer', () => {
	it('builds a prefix from characters typed close together', () => {
		let now = 0;
		const buffer = new TypeAheadBuffer(TYPE_AHEAD_RESET_MS, () => now);
		expect(buffer.push('r')).toBe('r');
		now += 300;
		expect(buffer.push('e')).toBe('re');
		now += 300;
		expect(buffer.push('p')).toBe('rep');
	});

	it('starts a new prefix after a pause', () => {
		let now = 0;
		const buffer = new TypeAheadBuffer(800, () => now);
		buffer.push('r');
		now += 801;
		expect(buffer.push('s')).toBe('s');
	});

	it('resets on request', () => {
		const buffer = new TypeAheadBuffer(800, () => 0);
		buffer.push('a');
		buffer.reset();
		expect(buffer.push('b')).toBe('b');
	});
});

async function listing() {
	const client = new FakeVfsClient();
	// Names sort alphabetically: a-0000 ... a-0599, then m-0000 ... m-0599, then z-0000 ... z-0599.
	const entries = ['a', 'm', 'z'].flatMap((letter, group) =>
		Array.from({ length: 600 }, (_, i) =>
			makeEntry(group * 1000 + i, `${letter}-${String(i).padStart(4, '0')}.txt`),
		),
	);
	client.setFolder(FOLDER, entries);
	const model = await openListingModel(client, FOLDER);
	return { client, model };
}

describe('findByPrefix', () => {
	it('finds a name in a page that was never loaded', async () => {
		const { model } = await listing();
		expect(model.cachedCount).toBe(0);
		expect(await findByPrefix(model, 'z-0010', 0)).toBe(1200 + 10);
	});

	it('ignores case', async () => {
		const { model } = await listing();
		expect(await findByPrefix(model, 'M-0001', 0)).toBe(601);
	});

	it('searches from the given position, then wraps round', async () => {
		const { model } = await listing();
		expect(await findByPrefix(model, 'm', 0)).toBe(600);
		expect(await findByPrefix(model, 'm', 601)).toBe(601);
		expect(await findByPrefix(model, 'a', 1500)).toBe(0);
	});

	it('includes the starting position itself', async () => {
		const { model } = await listing();
		expect(await findByPrefix(model, 'm-0000', 600)).toBe(600);
	});

	it('finds nothing for an absent prefix or an empty one', async () => {
		const { model } = await listing();
		expect(await findByPrefix(model, 'q', 0)).toBeNull();
		expect(await findByPrefix(model, '', 0)).toBeNull();
	});

	it('abandons the scan when cancelled, without reading every page', async () => {
		const { client, model } = await listing();
		const getRange = vi.spyOn(client, 'getRange');
		let calls = 0;
		const found = await findByPrefix(model, 'z', 0, () => ++calls > 2);
		expect(found).toBeNull();
		expect(getRange.mock.calls.length).toBeLessThan(4);
	});

	it('returns null for an empty listing', async () => {
		const client = new FakeVfsClient();
		client.setFolder(FOLDER, []);
		const model = await openListingModel(client, FOLDER);
		expect(await findByPrefix(model, 'a', 0)).toBeNull();
	});
});
