// Verifies what the ring derives from the queue: counts, urgency, overall progress and the freshest progress
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { fileLocation } from '../services/fakeVfsClient';
import { request } from '../test/opsHarness';
import {
	jobFraction,
	jobsTargetingDestination,
	mostUrgent,
	overallProgress,
	runningCount,
	unfinishedCount,
	withProgress,
} from './opsSelectors';

async function queue() {
	const fake = createFakeOpsClient({ concurrency: 4 });
	const ids = [];
	for (const name of ['a', 'b', 'c', 'd'])
		ids.push(await fake.submit(request([name], `/to-${name}`)));
	return { fake, ids: ids as [number, number, number, number] };
}

describe('mostUrgent', () => {
	it('is idle with no jobs and done with only finished ones', async () => {
		const { fake, ids } = await queue();
		expect(mostUrgent([])).toBe('idle');
		for (const id of ids) {
			fake.start(id);
			fake.done(id);
		}
		expect(mostUrgent(fake.jobs())).toBe('done');
	});

	it('puts waiting above failed above running above done', async () => {
		const { fake, ids } = await queue();
		const [a, b, c, d] = ids;
		fake.start(a);
		fake.done(a);
		expect(mostUrgent(fake.jobs())).toBe('running');
		fake.start(b);
		fake.fail(b, { kind: 'io', message: 'x' });
		expect(mostUrgent(fake.jobs())).toBe('failed');
		fake.start(c);
		fake.askConflicts(c, []);
		expect(mostUrgent(fake.jobs())).toBe('waiting');
		fake.start(d);
		expect(mostUrgent(fake.jobs())).toBe('waiting');
	});
});

describe('counts', () => {
	it('count running jobs apart from every unfinished one', async () => {
		const { fake, ids } = await queue();
		fake.start(ids[0]);
		fake.start(ids[1]);
		await fake.pause(ids[1]);
		fake.start(ids[2]);
		fake.done(ids[2]);
		expect(runningCount(fake.jobs())).toBe(1);
		expect(unfinishedCount(fake.jobs())).toBe(3);
	});
});

describe('overallProgress', () => {
	it('uses bytes when any job has a size, and ignores finished jobs', async () => {
		const { fake, ids } = await queue();
		fake.start(ids[0]);
		fake.start(ids[1]);
		fake.tick(ids[0], { bytesDone: 30, bytesTotal: 100, itemsDone: 1, itemsTotal: 1 });
		fake.tick(ids[1], { bytesDone: 10, bytesTotal: 100 });
		const jobs = () => fake.jobs().map((job) => withProgress(job, 0, undefined));
		expect(overallProgress(jobs()).fraction).toBeCloseTo(0.2);
		fake.done(ids[0]);
		expect(overallProgress(jobs()).bytesTotal).toBe(100 + 0 + 0);
	});

	it('falls back to items, and is null before anything is sized', () => {
		expect(overallProgress([]).fraction).toBeNull();
		expect(
			jobFraction({
				itemsDone: 1,
				itemsTotal: 4,
				bytesDone: 0,
				bytesTotal: 0,
				current: null,
				speedBps: 0,
				etaMs: null,
			}),
		).toBe(0.25);
	});
});

describe('withProgress', () => {
	it('prefers a tick newer than the job’s last event and otherwise the snapshot', async () => {
		const { fake, ids } = await queue();
		fake.start(ids[0]);
		const job = fake.jobs()[0]!;
		const tick = (revision: number, done: number) => ({
			job: ids[0],
			revision,
			progress: { ...job.progress, itemsDone: done },
			counts: { skipped: 0, failed: 0 },
		});
		expect(withProgress(job, 10, tick(11, 7)).progress.itemsDone).toBe(7);
		expect(withProgress(job, 10, tick(10, 7)).progress.itemsDone).toBe(0);
		expect(withProgress(job, 10, undefined).progress.itemsDone).toBe(0);
	});
});

describe('jobsTargetingDestination', () => {
	it('lists the unfinished jobs that put things in a folder', async () => {
		const { fake, ids } = await queue();
		fake.start(ids[0]);
		fake.done(ids[0]);
		expect(jobsTargetingDestination(fake.jobs(), fileLocation('/to-a'))).toEqual([]);
		expect(jobsTargetingDestination(fake.jobs(), fileLocation('/to-b')).map((j) => j.id)).toEqual([
			ids[1],
		]);
	});
});
