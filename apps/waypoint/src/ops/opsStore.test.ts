// Verifies the ops store mirrors the queue: the snapshot, held events, revision gating, gaps and progress
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { request } from '../test/opsHarness';
import type { OpsEvent } from '../services/opsClient';
import { createOpsStore } from './opsStore';
import { applyOpsEvent } from './opsApply';
import { overallProgress, withProgress } from './opsSelectors';

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

async function mirror(fake: FakeOpsClient) {
	const frames: Array<() => void> = [];
	const handle = createOpsStore(fake, {
		scheduleFrame: (flushFrame) => {
			frames.push(flushFrame);
			return () => frames.splice(frames.indexOf(flushFrame), 1);
		},
	});
	await handle.ready;
	return { handle, frames, state: () => handle.store.getState() };
}

describe('createOpsStore', () => {
	it('starts empty, then holds the snapshot', async () => {
		const fake = createFakeOpsClient();
		await fake.submit(request());
		const handle = createOpsStore(fake);
		expect(handle.store.getState().snapshot).toBeNull();
		await handle.ready;
		expect(handle.store.getState().snapshot?.jobs).toHaveLength(1);
		handle.dispose();
	});

	it('follows added, changed, removed and reordered jobs', async () => {
		const fake = createFakeOpsClient({ concurrency: 1 });
		const { handle, state } = await mirror(fake);
		const a = await fake.submit(request(['a']));
		const b = await fake.submit(request(['b']));
		const c = await fake.submit(request(['c']));
		fake.start(a);
		await fake.reorder(c, 0);
		expect(state().snapshot!.jobs.map((j) => j.id)).toEqual([a, c, b]);
		fake.done(a);
		await fake.dismiss(a);
		expect(state().snapshot!.jobs.map((j) => j.id)).toEqual([c, b]);
		expect(state().snapshot).toEqual(await fake.snapshot());
		handle.dispose();
	});

	it('applies events that arrive before the snapshot, and skips those it already holds', async () => {
		const fake = createFakeOpsClient();
		await fake.submit(request(['early']));
		const handle = createOpsStore(fake);
		// Between subscribing and the snapshot arriving, another job is added.
		const id = await fake.submit(request(['late']));
		await handle.ready;
		const jobs = handle.store.getState().snapshot!.jobs;
		expect(jobs.map((j) => j.id)).toContain(id);
		expect(jobs).toHaveLength(2);
		expect(handle.store.getState().resyncs).toBe(0);
		handle.dispose();
	});

	it('ignores a stale event', async () => {
		const fake = createFakeOpsClient();
		const { handle, state } = await mirror(fake);
		const id = await fake.submit(request());
		fake.start(id);
		const current = state().snapshot!;
		const stale: OpsEvent = {
			kind: 'jobChanged',
			job: { ...current.jobs[0]!, title: 'old' },
			revision: current.revision - 1,
		};
		expect(applyOpsEvent(current, stale).applied).toBe(false);
		expect(state().snapshot).toBe(current);
		handle.dispose();
	});

	it('does not take a gap in the revisions for a miss, since progress uses revisions up', async () => {
		const fake = createFakeOpsClient();
		const { handle, state } = await mirror(fake);
		const id = await fake.submit(request());
		fake.start(id);
		for (let i = 1; i <= 5; i++) fake.tick(id, { itemsDone: i, itemsTotal: 10 });
		fake.done(id);
		await flush();
		expect(state().resyncs).toBe(0);
		expect(state().snapshot!.jobs[0]!.state.state).toBe('done');
		handle.dispose();
	});

	it('reads a fresh snapshot when a change names a job it never heard of', async () => {
		const fake = createFakeOpsClient({ concurrency: 1 });
		const { handle, state } = await mirror(fake);
		fake.dropNextEvents(2); // the jobAdded and the queued change
		const id = await fake.submit(request(['missed']));
		expect(state().snapshot!.jobs).toHaveLength(0);
		fake.start(id);
		await flush();
		expect(state().resyncs).toBe(1);
		expect(state().snapshot).toEqual(await fake.snapshot());
		handle.dispose();
	});

	it('reads a fresh snapshot when the journal revision skips', async () => {
		const fake = createFakeOpsClient();
		const { handle, state } = await mirror(fake);
		const a = await fake.submit(request(['a']));
		fake.start(a);
		fake.done(a, 'one');
		const b = await fake.submit(request(['b']));
		fake.start(b);
		fake.dropNextEvents(3); // b's done, its undoable mark and the journal's second revision
		fake.done(b, 'two');
		expect(state().snapshot!.journal.revision).toBe(1);
		const c = await fake.submit(request(['c']));
		fake.start(c);
		fake.done(c, 'three'); // the journal's third revision skips the second
		await flush();
		expect(state().resyncs).toBe(1);
		expect(state().snapshot).toEqual(await fake.snapshot());
		expect(state().snapshot!.journal.undo?.label).toBe('three');
		handle.dispose();
	});

	it('keeps progress apart from the revisioned state and flushes it once per frame', async () => {
		const fake = createFakeOpsClient();
		const { handle, frames, state } = await mirror(fake);
		const id = await fake.submit(request());
		fake.start(id);
		const revision = state().snapshot!.revision;
		fake.tick(id, { itemsDone: 1, itemsTotal: 10 });
		fake.tick(id, { itemsDone: 2, itemsTotal: 10 });
		fake.tick(id, { itemsDone: 3, itemsTotal: 10 });
		// Nothing has reached the store yet, and the snapshot itself did not change.
		expect(handle.progress.getState().ticks[id]).toBeUndefined();
		expect(state().snapshot!.revision).toBe(revision);
		expect(frames).toHaveLength(1);
		frames[0]!();
		expect(handle.progress.getState().ticks[id]!.progress.itemsDone).toBe(3);
		const { job } = state().snapshot!.jobs.map((j) => ({ job: j }))[0]!;
		const fresh = withProgress(
			job,
			state().jobRevisions[id]!,
			handle.progress.getState().ticks[id],
		);
		expect(fresh.progress.itemsDone).toBe(3);
		handle.dispose();
	});

	it('drops a job’s progress when it finishes or is dismissed', async () => {
		const fake = createFakeOpsClient();
		const { handle, state } = await mirror(fake);
		const id = await fake.submit(request());
		fake.start(id);
		fake.tick(id, { itemsDone: 1, itemsTotal: 2 });
		handle.flushProgress();
		expect(handle.progress.getState().ticks[id]).toBeDefined();
		fake.done(id);
		expect(handle.progress.getState().ticks[id]).toBeUndefined();
		// A late tick for a finished job is not kept.
		expect(state().snapshot!.jobs[0]!.state.state).toBe('done');
		handle.dispose();
	});

	it('shows the progress of every unfinished job taken together', async () => {
		const fake = createFakeOpsClient();
		const { handle, state } = await mirror(fake);
		const a = await fake.submit(request(['a']));
		const b = await fake.submit(request(['b']));
		fake.start(a);
		fake.start(b);
		fake.tick(a, { bytesDone: 50, bytesTotal: 100 });
		fake.tick(b, { bytesDone: 0, bytesTotal: 100 });
		handle.flushProgress();
		const ticks = handle.progress.getState().ticks;
		const jobs = state().snapshot!.jobs.map((j) =>
			withProgress(j, state().jobRevisions[j.id]!, ticks[j.id]),
		);
		expect(overallProgress(jobs).fraction).toBeCloseTo(0.25);
		handle.dispose();
	});

	it('hands requests to the client: submitJob, undo and redo', async () => {
		const fake = createFakeOpsClient();
		const { handle } = await mirror(fake);
		const id = await handle.submitJob(request());
		fake.start(id);
		fake.done(id, 'Copy');
		const undoJob = await handle.undo();
		expect(undoJob).toBeGreaterThan(id);
		expect(fake.calls.map((c) => c[0])).toEqual(['submit', 'undo']);
		await expect(handle.redo()).rejects.toMatchObject({ kind: 'ops' });
		handle.dispose();
	});

	it('stops following after dispose', async () => {
		const fake = createFakeOpsClient();
		const { handle, state } = await mirror(fake);
		handle.dispose();
		await fake.submit(request());
		expect(state().snapshot!.jobs).toHaveLength(0);
	});

	it('does not throw when progress cannot be subscribed', async () => {
		const fake = createFakeOpsClient();
		vi.spyOn(fake, 'subscribeProgress').mockRejectedValue(new Error('no'));
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const handle = createOpsStore(fake);
		await handle.ready;
		expect(handle.store.getState().snapshot).not.toBeNull();
		expect(warn).toHaveBeenCalled();
		handle.dispose();
	});
});
