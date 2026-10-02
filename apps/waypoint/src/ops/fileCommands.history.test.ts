// Tests for `stepHistory`: one undo or redo of a history entry, waited for, with a refusal worded in plain words
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createRequest } from './fileCommands';
import { applyHistory, historyRows } from '../commands/historyCommands';
import { FOLDER } from '../test/browseHarness';
import { commandsHarness } from '../test/fileCommandsHarness';

const LOCATION = { display: '/home/test/report.txt', uri: 'file:///home/test/report.txt' };

/** Starts the job the harness's queue received as its `n`th and runs it with `end`. */
async function endJob(
	h: Awaited<ReturnType<typeof commandsHarness>>,
	n: number,
	end: (id: number) => void,
) {
	await vi.waitFor(() => expect(h.fake.jobs().length).toBeGreaterThanOrEqual(n));
	const job = h.fake.jobs()[n - 1]!;
	h.fake.start(job.id);
	end(job.id);
}

describe('stepHistory', () => {
	it('undoes one entry, waits for the job, and says it went through', async () => {
		const h = await commandsHarness();
		await h.ops.submitJob(createRequest('createFolder', FOLDER, 'New folder', 'main-1'));
		await h.finish(1, 'New folder');
		const [entry] = await h.commands.historyEntries();
		const step = h.commands.stepHistory('undo', entry!.id);
		await endJob(h, 2, (id) => h.fake.done(id));
		await expect(step).resolves.toEqual({ ok: true });
		expect(h.fake.calls.some(([name, id]) => name === 'undo' && id === entry!.id)).toBe(true);
		expect(h.said).toEqual([]);
	});

	it('says why the engine refused, in the words of the typed reason', async () => {
		const h = await commandsHarness();
		await h.ops.submitJob(createRequest('createFolder', FOLDER, 'New folder', 'main-1'));
		await h.finish(1, 'New folder');
		const [entry] = await h.commands.historyEntries();
		const step = h.commands.stepHistory('undo', entry!.id);
		await endJob(h, 2, (id) =>
			h.fake.fail(id, { kind: 'undoStale', reason: 'changed', location: LOCATION }),
		);
		await expect(step).resolves.toEqual({ ok: false, reason: 'report.txt has been changed since' });
	});

	it('says why a request the engine would not take was refused', async () => {
		const h = await commandsHarness();
		const outcome = await h.commands.stepHistory('redo', 99);
		expect(outcome).toEqual({ ok: false, reason: expect.any(String) });
		expect(outcome.ok).toBe(false);
	});

	it('chains with the palette’s rows: older entries go too, newest first, and a refusal stops the chain', async () => {
		const h = await commandsHarness();
		for (const [index, label] of ['First', 'Second', 'Third'].entries()) {
			await h.ops.submitJob(createRequest('createFolder', FOLDER, label, 'main-1'));
			await h.finish(index + 1, label);
		}
		const entries = await h.commands.historyEntries();
		const rows = historyRows(entries, { undoHead: entries[0]!.id, redoHead: null });
		const first = rows.find((row) => row.entry.label === 'First')!;
		expect(first.steps.map((step) => step.label)).toEqual(['Third', 'Second', 'First']);

		const report = applyHistory(first, { step: (kind, id) => h.commands.stepHistory(kind, id) });
		await endJob(h, 4, (id) => h.fake.done(id));
		await endJob(h, 5, (id) =>
			h.fake.fail(id, { kind: 'undoStale', reason: 'missing', location: LOCATION }),
		);
		const done = await report;
		expect(done.done.map((step) => step.label)).toEqual(['Third']);
		expect(done.stoppedAt?.label).toBe('Second');
		expect(done.text).toBe(
			'Undid 1 of 3 changes. Stopped at Second: report.txt is no longer where it was',
		);
		// One job per entry, newest first, and nothing was started for the third.
		expect(h.fake.calls.filter(([name]) => name === 'undo').map(([, id]) => id)).toEqual([
			entries[0]!.id,
			entries[1]!.id,
		]);
	});
});
