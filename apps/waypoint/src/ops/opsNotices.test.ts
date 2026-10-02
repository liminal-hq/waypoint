// Verifies the Undo toast after an undoable job and the notice for what start-up recovery found
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import type { NoticeAction } from '../app/notices';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { fileLocation } from '../services/fakeVfsClient';
import type { RecoveryReport } from '../services/opsClient';
import { request } from '../test/opsHarness';
import { recoveryText, runUndo, showRecoveryNotice, startUndoNotices } from './opsNotices';
import { createOpsStore } from './opsStore';

type Shown = { text: string; action?: NoticeAction };

async function setup() {
	const fake = createFakeOpsClient();
	const handle = createOpsStore(fake);
	await handle.ready;
	const shown: Shown[] = [];
	const show = (text: string, action?: NoticeAction) => {
		shown.push({ text, action });
	};
	return { fake, handle, shown, show };
}

const report = (labels: string[]): RecoveryReport => ({
	interrupted: labels.map((label, i) => ({
		job: i,
		kind: { kind: 'copy' },
		label,
		atMs: 0,
		planned: [],
		removed: [],
		restored: [],
		left: [],
	})),
	discarded: null,
	fromPrevious: false,
	repairs: [],
});

describe('the undo toast', () => {
	it('offers Undo when a job this window started finishes and can be undone', async () => {
		const { fake, handle, shown, show } = await setup();
		startUndoNotices(handle, { windowLabel: 'main-1', show });
		const id = await fake.submit(request(['a', 'b']));
		fake.start(id);
		fake.done(id, 'Copy 2 items');
		expect(shown).toHaveLength(1);
		expect(shown[0]!.text).toBe('Copied 2 items');
		expect(shown[0]!.action?.label).toBe('Undo');

		shown[0]!.action!.run();
		await vi.waitFor(() => expect(fake.calls.some((c) => c[0] === 'undo')).toBe(true));
		expect(fake.jobs().some((j) => j.kind.kind === 'undo')).toBe(true);
	});

	it('shows nothing for a job another window started, one that cannot be undone, or one that failed', async () => {
		const { fake, handle, shown, show } = await setup();
		startUndoNotices(handle, { windowLabel: 'main-2', show });
		const other = await fake.submit(request());
		fake.start(other);
		fake.done(other, 'Copy');
		const own = await fake.submit({ ...request(), originWindow: 'main-2' });
		fake.start(own);
		fake.done(own); // not undoable
		const bad = await fake.submit({ ...request(), originWindow: 'main-2' });
		fake.start(bad);
		fake.fail(bad, { kind: 'io', message: 'x' });
		expect(shown).toEqual([]);
	});

	it('does not announce jobs that were already done when it started, or an undo itself', async () => {
		const { fake, handle, shown, show } = await setup();
		const old = await fake.submit(request());
		fake.start(old);
		fake.done(old, 'Copy');
		await vi.waitFor(() => expect(handle.store.getState().snapshot?.jobs).toHaveLength(1));
		startUndoNotices(handle, { windowLabel: 'main-1', show });
		expect(shown).toEqual([]);
		const undo = await fake.undo();
		fake.start(undo);
		fake.done(undo);
		expect(shown).toEqual([]);
	});

	it('says why an undo was refused', async () => {
		const { handle, shown, show } = await setup();
		expect(await runUndo(handle, show)).toBeNull();
		expect(shown[0]!.text).toBe('Could not undo: There is nothing to undo');
	});
});

describe('the recovery notice', () => {
	it('names the interrupted operation', async () => {
		const fake = createFakeOpsClient();
		const shown: string[] = [];
		fake.setRecoveryReport(report(['Copy 3 items to Backups']));
		await showRecoveryNotice(fake, (text) => shown.push(text));
		expect(shown).toEqual(['An operation was interrupted: Copy 3 items to Backups']);
		// Rust hands it over once.
		await showRecoveryNotice(fake, (text) => shown.push(text));
		expect(shown).toHaveLength(1);
	});

	it('counts several, and says nothing when no job was interrupted', async () => {
		expect(recoveryText(report(['A', 'B']))).toBe('2 operations were interrupted, including: A');
		expect(recoveryText(report([]))).toBeNull();
		const fake = createFakeOpsClient();
		const shown: string[] = [];
		fake.setRecoveryReport({ ...report([]), repairs: ['x'] });
		await showRecoveryNotice(fake, (text) => shown.push(text));
		expect(shown).toEqual([]);
		void fileLocation;
	});
});

describe('the undo toast names its own job', () => {
	it('undoes the entry its job made, not the newest one another job made since', async () => {
		const { fake, handle, shown, show } = await setup();
		startUndoNotices(handle, { windowLabel: 'main-1', show });
		const first = await fake.submit(request(['a']));
		fake.start(first);
		fake.done(first, 'Copy a');
		// Another job, in another window, finishes after it.
		const second = await fake.submit({ ...request(['b']), originWindow: 'main-2' });
		fake.start(second);
		fake.done(second, 'Copy b');
		const entries = (await fake.journalSummaries()).map((s) => s.id);
		expect(entries).toHaveLength(2);

		shown[0]!.action!.run();
		await vi.waitFor(() => expect(fake.calls.some((c) => c[0] === 'undo')).toBe(true));
		const undo = fake.calls.find((c) => c[0] === 'undo')!;
		const firstEntry = await fake.journalEntryOf(first);
		expect(undo[1]).toBe(firstEntry);
		expect(undo[1]).not.toBe(await fake.journalEntryOf(second));
		const job = fake.jobs().find((j) => j.kind.kind === 'undo')!;
		expect(job.kind).toEqual({ kind: 'undo', of: firstEntry });
	});

	it('says so in plain words when its entry was already undone, and undoes nothing else', async () => {
		const { fake, handle, shown, show } = await setup();
		startUndoNotices(handle, { windowLabel: 'main-1', show });
		const first = await fake.submit(request(['a']));
		fake.start(first);
		fake.done(first, 'Copy a');
		const other = await fake.submit({ ...request(['b']), originWindow: 'main-2' });
		fake.start(other);
		fake.done(other, 'Copy b');
		// The first job's work is undone by another route, then its toast is clicked.
		const undo = await fake.undo((await fake.journalEntryOf(first))!);
		fake.start(undo);
		fake.done(undo);
		fake.calls.length = 0;

		shown[0]!.action!.run();
		await vi.waitFor(() => expect(shown).toHaveLength(2));
		expect(shown[1]!.text).toBe(
			'Could not undo: That change was already undone or is no longer in the history',
		);
		expect(fake.jobs().filter((j) => j.kind.kind === 'undo')).toHaveLength(1);
	});

	it('does not undo anything when the job has no entry', async () => {
		const { fake, handle, shown, show } = await setup();
		const job = await fake.submit(request(['a']));
		fake.start(job);
		fake.done(job, 'Copy a');
		const other = await fake.submit(request(['b']));
		fake.start(other);
		fake.done(other, 'Copy b');
		const { undoJob } = await import('./opsNotices');
		expect(await undoJob(handle, show, 9999)).toBeNull();
		expect(shown[0]!.text).toContain('no longer in the history');
		expect(fake.calls.some((c) => c[0] === 'undo')).toBe(false);
	});
});
