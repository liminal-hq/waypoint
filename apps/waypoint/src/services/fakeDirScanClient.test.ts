// Tests for the in-memory DirScanClient: the contract Overview is built against
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { DirScanEvent } from '@liminal-hq/waypoint-protocol/generated/DirScanEvent';
import { FakeDirScanClient, REMAINDER_NAME, fakeDirScanResult } from './fakeDirScanClient';

const home = { display: '/home/a', uri: 'file:///home/a' };

function setup() {
	const client = new FakeDirScanClient();
	const events: DirScanEvent[] = [];
	return { client, events, onEvent: (e: DirScanEvent) => events.push(e) };
}

describe('FakeDirScanClient', () => {
	it('builds results whose shares add up and whose remainder is last', () => {
		const result = fakeDirScanResult(home, [
			['Pictures', 750],
			['Music', 150],
			[null, 100],
		]);
		expect(result.totalBytes).toBe(1000);
		expect(result.rows.map((r) => r.share)).toEqual([0.75, 0.15, 0.1]);
		expect(result.rows[2]).toMatchObject({ kind: 'other', name: REMAINDER_NAME, location: null });
	});

	it('sends partials then done, and a finished scan becomes the cached result', async () => {
		const { client, events, onEvent } = setup();
		expect(await client.cached(home)).toBeNull();
		const run = await client.scan(home, onEvent, { allocated: true });
		expect(client.lastOptions).toEqual({ allocated: true });
		const first = fakeDirScanResult(home, [['A', 10]]);
		const all = fakeDirScanResult(
			home,
			[
				['A', 10],
				['B', 5],
			],
			{ measuredAtMs: 99 },
		);
		client.progress(run.job, 'B', 12);
		client.partial(run.job, first);
		client.finish(run.job, all);
		expect(events.map((e) => e.kind)).toEqual(['progress', 'partial', 'done']);
		expect(await client.cached(home)).toBe(all);
		// Nothing more is sent once it has ended.
		client.partial(run.job, first);
		await run.cancel();
		expect(events).toHaveLength(3);
	});

	it('cancels with the latest partial result and does not cache it', async () => {
		const { client, events, onEvent } = setup();
		const run = await client.scan(home, onEvent);
		const first = fakeDirScanResult(home, [['A', 10]]);
		client.partial(run.job, first);
		await run.cancel();
		expect(events.at(-1)).toEqual({ kind: 'cancelled', result: first });
		expect(await client.cached(home)).toBeNull();
	});

	it('serves a seeded cache, and fails a scan on request', async () => {
		const { client, events, onEvent } = setup();
		const cached = fakeDirScanResult(home, [['A', 1]], { measuredAtMs: 5 });
		client.setCached(cached);
		expect(await client.cached(home)).toBe(cached);
		client.failNextScan({ kind: 'staleHandle' });
		await expect(client.scan(home, onEvent)).rejects.toEqual({ kind: 'staleHandle' });
		const run = await client.scan(home, onEvent);
		client.fail(run.job, { kind: 'staleHandle' });
		expect(events.at(-1)).toEqual({ kind: 'failed', error: { kind: 'staleHandle' } });
	});
});
