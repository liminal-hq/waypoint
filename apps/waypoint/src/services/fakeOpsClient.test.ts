// Verifies the fake queue keeps the Rust state machine: the legal moves, and each command's refusals
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { JobState } from '@liminal-hq/waypoint-protocol/generated/JobState';
import { describe, expect, it } from 'vitest';
import {
	createFakeOpsClient,
	FakeOpsClient,
	isLegalTransition,
	LEGAL_TRANSITIONS,
} from './fakeOpsClient';
import type { Clipboard } from './opsClient';
import { request } from '../test/opsHarness';
import { fileLocation } from './fakeVfsClient';

// The table in the header of the Rust queue is the source of truth; this reads it from disk.
const queueSource = readFileSync(
	join(import.meta.dirname, '../../../../crates/waypoint-ops/src/queue.rs'),
	'utf8',
);

function rustTable(): Record<string, string[]> {
	const table: Record<string, string[]> = {};
	for (const line of queueSource.split('\n')) {
		const match = /^\/\/\s+(\w+)\s+->\s+([\w |]+)$/.exec(line);
		if (match) table[match[1]!] = match[2]!.split('|').map((name) => name.trim());
	}
	return table;
}

const STATES: JobState['state'][] = [
	'planning',
	'queued',
	'running',
	'paused',
	'waiting',
	'cancelling',
	'cancelled',
	'done',
	'failed',
];

describe('the legal transitions', () => {
	it('are the table at the top of crates/waypoint-ops/src/queue.rs', () => {
		const rust = rustTable();
		// The comment lists the states with moves; the final ones have none.
		for (const state of STATES) {
			expect(LEGAL_TRANSITIONS[state], state).toEqual(rust[state] ?? []);
		}
	});

	it('allow exactly the listed moves and no other', () => {
		for (const from of STATES) {
			for (const to of STATES) {
				expect(isLegalTransition(from, to), `${from} -> ${to}`).toBe(
					LEGAL_TRANSITIONS[from]!.includes(to),
				);
			}
		}
	});
});

describe('FakeOpsClient', () => {
	it('plans a submitted job at once and leaves it queued until a worker starts it', async () => {
		const fake = createFakeOpsClient();
		const id = await fake.submit(request(['a', 'b']));
		const job = fake.jobs()[0]!;
		expect(job.id).toBe(id);
		expect(job.state.state).toBe('queued');
		expect(job.sources).toEqual({ count: 2, first: 'a' });
		fake.start(id);
		expect(fake.jobs()[0]!.state.state).toBe('running');
	});

	it('can hold a job in planning until it is planned', async () => {
		const fake = createFakeOpsClient({ autoPlan: false });
		const id = await fake.submit(request());
		expect(fake.jobs()[0]!.state.state).toBe('planning');
		await fake.cancel(id);
		expect(fake.jobs()[0]!.state.state).toBe('cancelled');
		// A job cancelled while it was planned stays cancelled.
		fake.planned(id);
		expect(fake.jobs()[0]!.state.state).toBe('cancelled');
	});

	it('refuses what the table does not allow and changes nothing', async () => {
		const fake = createFakeOpsClient();
		const id = await fake.submit(request());
		await expect(fake.pause(id)).rejects.toMatchObject({ kind: 'queue' });
		await expect(fake.resume(id)).rejects.toMatchObject({ kind: 'queue' });
		await expect(fake.dismiss(id)).rejects.toMatchObject({ kind: 'queue' });
		await expect(fake.retry(id)).rejects.toMatchObject({ kind: 'queue' });
		await expect(fake.pause(999 as never)).rejects.toMatchObject({ kind: 'queue' });
		expect(fake.jobs()[0]!.state.state).toBe('queued');
	});

	it('pauses and resumes a running job, and cancels through cancelling', async () => {
		const fake = createFakeOpsClient();
		const id = await fake.submit(request());
		fake.start(id);
		await fake.pause(id);
		expect(fake.jobs()[0]!.state.state).toBe('paused');
		await fake.resume(id);
		await fake.cancel(id);
		expect(fake.jobs()[0]!.state.state).toBe('cancelling');
		// A second cancel changes nothing.
		await fake.cancel(id);
		fake.settleCancel(id);
		expect(fake.jobs()[0]!.state.state).toBe('cancelled');
	});

	it('refuses a start when every worker slot is taken', async () => {
		const fake = createFakeOpsClient({ concurrency: 1 });
		const a = await fake.submit(request());
		const b = await fake.submit(request());
		fake.start(a);
		expect(() => fake.start(b)).toThrow();
		fake.done(a);
		fake.start(b);
		expect(fake.jobs()[1]!.state.state).toBe('running');
	});

	it('starts queued jobs as slots free when autoStart is on', async () => {
		const fake = createFakeOpsClient({ concurrency: 1, autoStart: true });
		const a = await fake.submit(request());
		await fake.submit(request());
		expect(fake.jobs().map((j) => j.state.state)).toEqual(['running', 'queued']);
		fake.done(a);
		expect(fake.jobs().map((j) => j.state.state)).toEqual(['done', 'running']);
	});

	it('retries a failed or cancelled job as a new job and dismisses only finished ones', async () => {
		const fake = createFakeOpsClient();
		const id = await fake.submit(request(['x']));
		fake.start(id);
		fake.fail(id, { kind: 'io', message: 'boom' });
		const again = await fake.retry(id);
		expect(again).not.toBe(id);
		expect(fake.jobs().map((j) => j.id)).toEqual([id, again]);
		await fake.dismiss(id);
		expect(fake.jobs().map((j) => j.id)).toEqual([again]);
		await fake.dismissFinished();
		expect(fake.jobs()).toHaveLength(1);
	});

	it('reorders only queued jobs, among the queued ones', async () => {
		const fake = createFakeOpsClient({ concurrency: 1 });
		const a = await fake.submit(request(['a']));
		const b = await fake.submit(request(['b']));
		const c = await fake.submit(request(['c']));
		fake.start(a);
		await expect(fake.reorder(a, 0)).rejects.toMatchObject({ kind: 'queue' });
		await fake.reorder(c, 0);
		expect(fake.jobs().map((j) => j.id)).toEqual([a, c, b]);
		await fake.reorder(c, 99);
		expect(fake.jobs().map((j) => j.id)).toEqual([a, b, c]);
	});

	it('waits on conflicts, keeps the unanswered ones, and runs once all are answered', async () => {
		const fake = createFakeOpsClient();
		const id = await fake.submit(request(['a', 'b']));
		fake.start(id);
		const conflict = (name: string) => ({
			source: fileLocation(`/src/${name}`),
			existing: fileLocation(`/dest/${name}`),
			name,
			kind: 'fileOverFile' as const,
			withinBatch: false,
			sourceSize: 1,
			existingSize: 2,
			sourceModifiedMs: 1,
			existingModifiedMs: 2,
		});
		fake.askConflicts(id, [conflict('a'), conflict('b')]);
		await fake.resolve(id, [{ source: fileLocation('/src/a'), policy: 'skip' }]);
		const waiting = fake.jobs()[0]!.state;
		expect(waiting).toMatchObject({ state: 'waiting', reason: { kind: 'conflicts' } });
		if (waiting.state === 'waiting' && waiting.reason.kind === 'conflicts') {
			expect(waiting.reason.conflicts.map((c) => c.name)).toEqual(['b']);
		}
		await fake.resolve(id, [], 'keepBoth');
		expect(fake.jobs()[0]!.state.state).toBe('running');
		await expect(fake.resolve(id, [])).rejects.toMatchObject({ kind: 'queue' });
	});

	it('answers an error with a decision: retry and skip run again, cancel unwinds', async () => {
		const fake = createFakeOpsClient();
		const id = await fake.submit(request());
		fake.start(id);
		const item = fileLocation('/src/a.txt');
		fake.askError(id, { kind: 'permissionDenied', location: item }, item);
		await fake.resolveError(id, 'skip');
		expect(fake.jobs()[0]!.state.state).toBe('running');
		fake.askError(id, { kind: 'permissionDenied', location: item }, item);
		await fake.resolveError(id, 'cancel');
		expect(fake.jobs()[0]!.state.state).toBe('cancelling');
	});

	it('records an undoable job in the journal and undoes then redoes it as jobs', async () => {
		const fake = createFakeOpsClient();
		await expect(fake.undo()).rejects.toMatchObject({ kind: 'ops' });
		const id = await fake.submit(request());
		fake.start(id);
		fake.done(id, 'Copy 1 item');
		expect(fake.jobs()[0]!.undoable).toBe(true);
		expect((await fake.snapshot()).journal.undo?.label).toBe('Copy 1 item');

		const undoJob = await fake.undo();
		fake.start(undoJob);
		fake.done(undoJob);
		let journal = (await fake.snapshot()).journal;
		expect(journal.undo).toBeNull();
		expect(journal.redo?.label).toBe('Copy 1 item');

		const redoJob = await fake.redo();
		fake.start(redoJob);
		fake.done(redoJob);
		journal = (await fake.snapshot()).journal;
		expect(journal.undo?.label).toBe('Copy 1 item');
		expect(journal.redo).toBeNull();
	});

	it('keeps a shared clipboard with its source, clears it, and refuses an empty selection', async () => {
		const fake = new FakeOpsClient({
			resolveSelection: (_handle, spec) =>
				spec.ids.map((id) => ({ display: `/a/${id}`, uri: `file:///a/${id}` })),
		});
		const heard: Clipboard[] = [];
		fake.onClipboard((c) => heard.push(c));
		expect(await fake.getClipboard()).toEqual({
			mode: 'copy',
			items: [],
			source: 'app',
			revision: 0,
		});
		const set = await fake.setClipboardFromSelection(1, { kind: 'some', ids: [7, 8] }, 'cut');
		expect(set).toMatchObject({ mode: 'cut', source: 'app', revision: 1 });
		expect(set.items.map((item) => item.uri)).toEqual(['file:///a/7', 'file:///a/8']);
		const adopted = await fake.setClipboard('copy', set.items, 'os');
		expect(adopted).toMatchObject({ source: 'os', revision: 2 });
		await fake.setClipboard('copy', []);
		expect(heard.map((c) => c.revision)).toEqual([1, 2, 3]);
		await expect(
			fake.setClipboardFromSelection(1, { kind: 'some', ids: [] }, 'copy'),
		).rejects.toMatchObject({ kind: 'ops', error: { kind: 'unsupported' } });
		expect((await fake.getClipboard()).revision).toBe(3);
	});

	it('hands over the recovery report once', async () => {
		const fake = createFakeOpsClient();
		fake.setRecoveryReport({ interrupted: [], discarded: null, fromPrevious: false, repairs: [] });
		expect(await fake.takeRecoveryReport()).not.toBeNull();
		expect(await fake.takeRecoveryReport()).toBeNull();
	});

	it('sends progress on the channel with a revision, and no event', async () => {
		const fake = createFakeOpsClient();
		const ticks: number[] = [];
		const events: string[] = [];
		await fake.subscribeProgress((tick) => ticks.push(tick.revision));
		fake.onEvent((event) => events.push(event.kind));
		const id = await fake.submit(request());
		fake.start(id);
		const before = events.length;
		fake.tick(id, { itemsDone: 1, itemsTotal: 4 });
		expect(events).toHaveLength(before);
		expect(ticks).toHaveLength(1);
		expect((await fake.snapshot()).revision).toBe(ticks[0]);
	});
});
