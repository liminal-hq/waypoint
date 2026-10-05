// Verifies the announcements: starts and endings with counts, a few steps of long jobs, and never every tick
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { fileLocation } from '../services/fakeVfsClient';
import { request } from '../test/opsHarness';
import { MILESTONE_AFTER_MS, startOpsAnnouncer } from './opsAnnouncer';
import { createOpsStore } from './opsStore';

async function setup(options: { include?: (job: { originWindow: string }) => boolean } = {}) {
	let clock = 1_000;
	const fake = createFakeOpsClient({ concurrency: 4, now: () => clock });
	const handle = createOpsStore(fake, {
		scheduleFrame: (flush) => {
			flush();
			return () => {};
		},
	});
	await handle.ready;
	const said: string[] = [];
	startOpsAnnouncer(handle, {
		announce: (text) => said.push(text),
		now: () => clock,
		...options,
	});
	return { fake, handle, said, advance: (ms: number) => (clock += ms) };
}

describe('the ops announcer', () => {
	it('announces a job starting once, with how many others are in progress', async () => {
		const { fake, said } = await setup();
		const a = await fake.submit(request(['a']));
		const b = await fake.submit(request(['b']));
		expect(said).toEqual([]); // queued is not started
		fake.start(a);
		// The second job is queued, so one other is in progress.
		expect(said).toEqual(['Started: Copying a 1 operation still in progress']);
		fake.start(b);
		expect(said[1]).toBe('Started: Copying b 1 operation still in progress');
		await fake.pause(b);
		await fake.resume(b); // running again is not a new start
		expect(said).toHaveLength(2);
	});

	it('announces the end of a job: finished, failed, cancelled', async () => {
		const { fake, said } = await setup();
		const [a, b, c] = [
			await fake.submit(request(['a'])),
			await fake.submit(request(['b'])),
			await fake.submit(request(['c'])),
		];
		fake.start(a);
		fake.start(b);
		fake.start(c);
		said.length = 0;
		fake.done(a);
		fake.fail(b, { kind: 'permissionDenied', location: fileLocation('/src/b') });
		await fake.cancel(c);
		fake.settleCancel(c);
		expect(said).toEqual([
			'Copied a. 2 operations still in progress',
			'Failed: Copying b. Permission denied for b 1 operation still in progress',
			'Cancelled: Copying c',
		]);
	});

	it('says when a job waits for the person', async () => {
		const { fake, said } = await setup();
		const a = await fake.submit(request(['a']));
		fake.start(a);
		said.length = 0;
		fake.askConflicts(a, []);
		expect(said).toEqual(['Copying a is waiting for you']);
	});

	it('says once that a job lost its server, however many times it tries again', async () => {
		const { fake, said } = await setup();
		const a = await fake.submit(request(['a']));
		fake.start(a);
		said.length = 0;
		const lost = {
			kind: 'connection' as const,
			error: { kind: 'disconnected' as const, location: fileLocation('/srv/a') },
		};
		fake.offline(a, lost, fileLocation('/a'), 1);
		fake.online(a);
		fake.offline(a, lost, fileLocation('/a'), 2);
		expect(said).toEqual(['Copying a lost its connection and will try again by itself']);
	});

	it('speaks 25, 50 and 75 % of a long job only, at most one message per update', async () => {
		const { fake, handle, said, advance } = await setup();
		const a = await fake.submit(request(['a', 'b', 'c', 'd']));
		fake.start(a);
		said.length = 0;
		advance(MILESTONE_AFTER_MS);
		fake.tick(a, { itemsDone: 1, itemsTotal: 4 });
		expect(said).toEqual(['Copying 4 items: 25% done, 1 of 4 items']);
		fake.tick(a, { itemsDone: 1, itemsTotal: 4 }); // no new tick data: nothing
		fake.tick(a, { itemsDone: 2, itemsTotal: 4, current: 'x' });
		fake.tick(a, { itemsDone: 3, itemsTotal: 4, current: 'y' });
		expect(said).toHaveLength(3);
		expect(said[2]).toContain('75%');
		fake.tick(a, { itemsDone: 4, itemsTotal: 4 }); // 100 % is the ending, not a step
		expect(said).toHaveLength(3);
		handle.dispose();
	});

	it('skips steps all at once when one update crosses several', async () => {
		const { fake, said, advance } = await setup();
		const a = await fake.submit(request(['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h']));
		fake.start(a);
		said.length = 0;
		advance(MILESTONE_AFTER_MS);
		fake.tick(a, { itemsDone: 6, itemsTotal: 8 });
		expect(said).toEqual(['Copying 8 items: 75% done, 6 of 8 items']);
	});

	it('is silent about the steps of a quick job, and does not announce them later either', async () => {
		const { fake, said, advance } = await setup();
		const a = await fake.submit(request(['a', 'b', 'c', 'd']));
		fake.start(a);
		said.length = 0;
		fake.tick(a, { itemsDone: 1, itemsTotal: 4 }); // 25 %, at once
		advance(MILESTONE_AFTER_MS + 1);
		fake.tick(a, { itemsDone: 1, itemsTotal: 4, current: 'again' }); // still 25 %
		expect(said).toEqual([]);
	});

	it('speaks only for the jobs it is asked to', async () => {
		const { fake, said } = await setup({ include: (job) => job.originWindow === 'other' });
		const a = await fake.submit(request(['a']));
		fake.start(a);
		fake.done(a);
		expect(said).toEqual([]);
	});

	it('does not announce what was already in the queue when it started', async () => {
		const fake = createFakeOpsClient();
		const a = await fake.submit(request(['a']));
		fake.start(a);
		const handle = createOpsStore(fake);
		await handle.ready;
		const said: string[] = [];
		startOpsAnnouncer(handle, { announce: (text) => said.push(text) });
		fake.done(a);
		expect(said).toEqual(['Copied a.']);
		expect(said).not.toContain('Started: Copying a');
	});
});
