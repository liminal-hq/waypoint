// Tests for the history rows: which entries are offered, what choosing one undoes or redoes and in what order, and how a refusal is reported
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { t } from '../i18n/messages';
import type { HistoryStepOutcome } from '../ops/fileCommands';
import { entry } from '../test/commandFacts';
import {
	applyHistory,
	confirmSpec,
	historyRowKey,
	historyRows,
	needsConfirmation,
	parseHistoryRowKey,
	relativeTime,
	sameChain,
	type HistoryKind,
} from './historyCommands';

// Newest first, as the journal lists it: 5 and 4 and 3 are applied, 2 and 1 were undone.
const history = [
	entry(5, 'Move 3 items to Trash'),
	entry(4, 'New folder'),
	entry(3, 'Rename report'),
	entry(2, 'Duplicate notes', { undoable: false, redoable: true }),
	entry(1, 'Copy photos', { undoable: false, redoable: true }),
];
const heads = { undoHead: 5, redoHead: 1 };

describe('which entries are rows', () => {
	it('offers every entry but the two the registry’s Undo and Redo act on', () => {
		expect(historyRows(history, heads).map((row) => row.key)).toEqual([
			historyRowKey('undo', 4),
			historyRowKey('undo', 3),
			historyRowKey('redo', 2),
		]);
	});

	it('offers nothing for an empty history, and nothing for the heads alone', () => {
		expect(historyRows([], { undoHead: null, redoHead: null })).toEqual([]);
		expect(historyRows([history[0]!], { undoHead: 5, redoHead: null })).toEqual([]);
	});

	it('reads a key back, and rejects any other id', () => {
		expect(parseHistoryRowKey('history:undo:4')).toEqual({ kind: 'undo', entry: 4 });
		expect(parseHistoryRowKey('history:redo:12')).toEqual({ kind: 'redo', entry: 12 });
		expect(parseHistoryRowKey('command:undo')).toBeNull();
	});
});

describe('what choosing an undo row undoes', () => {
	const rows = historyRows(history, heads);
	const row = (kind: HistoryKind, id: number) =>
		rows.find((candidate) => candidate.key === historyRowKey(kind, id))!;

	it('undoes the chosen entry and every applied entry newer than it, newest first', () => {
		expect(row('undo', 3).steps.map((step) => step.id)).toEqual([5, 4, 3]);
		expect(row('undo', 4).steps.map((step) => step.id)).toEqual([5, 4]);
	});

	it('words a longer chain with how far back it goes, and a single entry plainly', () => {
		expect(row('undo', 3).label).toBe('Undo 3 changes back to: Rename report');
		expect(row('undo', 4).label).toBe('Undo 2 changes back to: New folder');
	});

	it('skips entries that are already undone between the head and the chosen one', () => {
		const mixed = [
			entry(4, 'Move'),
			entry(3, 'Undone one', { undoable: false, redoable: true }),
			entry(2, 'Rename'),
		];
		const [only] = historyRows(mixed, { undoHead: 4, redoHead: 3 });
		expect(only!.steps.map((step) => step.id)).toEqual([4, 2]);
	});
});

describe('what choosing a redo row redoes', () => {
	const redoable = [
		entry(6, 'Newest', { undoable: false, redoable: true }),
		entry(5, 'Middle', { undoable: false, redoable: true }),
		entry(4, 'Oldest', { undoable: false, redoable: true }),
	];

	it('redoes the undone entries older than the chosen one first, oldest first, ending with it', () => {
		const rows = historyRows(redoable, { undoHead: null, redoHead: 4 });
		expect(rows.map((row) => [row.entry.id, row.steps.map((step) => step.id)])).toEqual([
			[5, [4, 5]],
			[6, [4, 5, 6]],
		]);
		expect(rows[1]!.label).toBe('Redo 3 changes up to: Newest');
	});

	it('words a single redo plainly', () => {
		const [row] = historyRows(history, heads).filter((candidate) => candidate.kind === 'redo');
		expect(row!.steps.map((step) => step.id)).toEqual([1, 2]);
		// With no head to leave to the registry's Redo, the oldest undone entry is a single step.
		const single = historyRows(redoable, { undoHead: null, redoHead: null });
		expect(single[0]!.label).toBe('Redo: Oldest');
	});
});

describe('confirming', () => {
	const undo = (rows: ReturnType<typeof historyRows>, id: number) =>
		rows.find((row) => row.key === historyRowKey('undo', id))!;

	it('asks when more than one change would go, and not for a single one', () => {
		const rows = historyRows([entry(3, 'a'), entry(2, 'b'), entry(1, 'c')], {
			undoHead: 3,
			redoHead: null,
		});
		expect(needsConfirmation(undo(rows, 2))).toBe(true);
		expect(needsConfirmation(undo(rows, 1))).toBe(true);
		const one = historyRows([entry(2, 'a', { undoable: false, redoable: true }), entry(1, 'b')], {
			undoHead: 1,
			redoHead: 2,
		});
		expect(one).toEqual([]);
	});

	it('asks to finish an undo that stopped part way, even for one change', () => {
		const partly = historyRows(
			[
				entry(2, 'Newer', { undoable: false, redoable: true }),
				entry(1, 'Stopped', { partlyUndone: true }),
			],
			{ undoHead: null, redoHead: 2 },
		);
		const row = undo(partly, 1);
		expect(row.steps).toHaveLength(1);
		expect(row.partly).toBe(true);
		expect(needsConfirmation(row)).toBe(true);
		const spec = confirmSpec(row);
		expect(spec.title).toBe(t('history.confirm.partly.title'));
		expect(spec.items).toEqual(['Stopped']);
	});

	it('lists what goes, in order, with a destructive button so focus starts on Cancel', () => {
		const rows = historyRows(history, heads);
		const spec = confirmSpec(undo(rows, 3));
		expect(spec.title).toBe('Undo 3 changes?');
		expect(spec.items).toEqual(['Move 3 items to Trash', 'New folder', 'Rename report']);
		expect(spec.confirmLabel).toBe(t('history.confirm.undo.confirm'));
		expect(spec.danger).toBe(true);
	});

	it('notes a partly undone entry inside a longer chain', () => {
		const rows = historyRows([entry(3, 'Newest'), entry(2, 'Stopped', { partlyUndone: true })], {
			undoHead: 3,
			redoHead: null,
		});
		const spec = confirmSpec(undo(rows, 2));
		expect(spec.note).toBe(t('history.confirm.partly.note'));
	});

	it('mirrors for a redo', () => {
		const rows = historyRows(
			[
				entry(2, 'Newer', { undoable: false, redoable: true }),
				entry(1, 'Older', { undoable: false, redoable: true }),
			],
			{ undoHead: null, redoHead: 1 },
		);
		const spec = confirmSpec(rows[0]!);
		expect(spec.title).toBe('Redo 2 changes?');
		expect(spec.items).toEqual(['Older', 'Newer']);
		expect(spec.confirmLabel).toBe(t('history.confirm.redo.confirm'));
	});
});

describe('applying a chain', () => {
	const rows = historyRows(history, heads);
	const chain = rows.find((row) => row.key === historyRowKey('undo', 3))!;

	it('runs one step per entry, in order, each after the one before, and says all went through', async () => {
		const order: string[] = [];
		const step = vi.fn(async (kind: HistoryKind, id: number): Promise<HistoryStepOutcome> => {
			order.push(`start ${kind} ${id}`);
			await Promise.resolve();
			order.push(`end ${kind} ${id}`);
			return { ok: true };
		});
		const report = await applyHistory(chain, { step });
		expect(order).toEqual([
			'start undo 5',
			'end undo 5',
			'start undo 4',
			'end undo 4',
			'start undo 3',
			'end undo 3',
		]);
		expect(report.stoppedAt).toBeNull();
		expect(report.done.map((done) => done.id)).toEqual([5, 4, 3]);
		expect(report.text).toBe('Undid 3 changes, back to: Rename report');
	});

	it('stops at the first refusal and says what was undone and why it stopped', async () => {
		const step = vi.fn(async (_kind: HistoryKind, id: number): Promise<HistoryStepOutcome> =>
			id === 4 ? { ok: false, reason: 'New folder has been changed since' } : { ok: true },
		);
		const report = await applyHistory(chain, { step });
		expect(step.mock.calls.map(([, id]) => id)).toEqual([5, 4]);
		expect(report.done.map((done) => done.id)).toEqual([5]);
		expect(report.stoppedAt?.id).toBe(4);
		expect(report.text).toBe(
			'Undid 1 of 3 changes. Stopped at New folder: New folder has been changed since',
		);
	});

	it('says nothing was undone when the very first is refused', async () => {
		const step = vi.fn(async (): Promise<HistoryStepOutcome> => ({
			ok: false,
			reason: 'it is gone',
		}));
		const report = await applyHistory(chain, { step });
		expect(step).toHaveBeenCalledTimes(1);
		expect(report.done).toEqual([]);
		expect(report.text).toBe(
			'Nothing was undone. Move 3 items to Trash could not be undone: it is gone',
		);
	});

	it('mirrors for a redo: oldest first, and a refusal is worded for redo', async () => {
		const redo = rows.find((row) => row.key === historyRowKey('redo', 2))!;
		const step = vi.fn(async (_kind: HistoryKind, id: number): Promise<HistoryStepOutcome> =>
			id === 2 ? { ok: false, reason: 'the name is taken' } : { ok: true },
		);
		const report = await applyHistory(redo, { step });
		expect(step.mock.calls.map(([kind, id]) => [kind, id])).toEqual([
			['redo', 1],
			['redo', 2],
		]);
		expect(report.text).toBe('Redid 1 of 2 changes. Stopped at Duplicate notes: the name is taken');
	});

	it('says a single undo plainly', async () => {
		const single = historyRows([entry(1, 'Only')], { undoHead: null, redoHead: null });
		const report = await applyHistory(single[0]!, { step: async () => ({ ok: true }) });
		expect(report.text).toBe('Undid Only');
	});
});

describe('relative times', () => {
	const now = new Date(2026, 9, 1, 14, 30).getTime();
	const ago = (ms: number) => relativeTime(now - ms, now, 'en-CA');
	const MINUTE = 60_000;
	const HOUR = 60 * MINUTE;
	const DAY = 24 * HOUR;

	it('says now, minutes, hours and days with Intl.RelativeTimeFormat', () => {
		expect(ago(5_000)).toBe('now');
		expect(ago(2 * MINUTE)).toBe('2 minutes ago');
		expect(ago(MINUTE)).toBe('1 minute ago');
		expect(ago(3 * HOUR)).toBe('3 hours ago');
		expect(ago(DAY)).toBe('yesterday');
		expect(ago(3 * DAY)).toBe('3 days ago');
	});

	it('falls back to the date and time in the system’s clock after a week', () => {
		const old = now - 10 * DAY;
		expect(relativeTime(old, now, 'en-CA', 'h23')).toMatch(/14:30/);
		expect(relativeTime(old, now, 'en-CA', 'h12')).toMatch(/2:30/);
	});

	it('follows the locale', () => {
		expect(relativeTime(now - 2 * MINUTE, now, 'fr-CA')).toMatch(/2 minutes/);
		expect(relativeTime(now - 2 * MINUTE, now, 'fr-CA')).not.toBe('2 minutes ago');
	});
});

describe('sameChain', () => {
	const heads = { undoHead: null, redoHead: null };
	const older = entry(1, 'Older');
	const newer = entry(2, 'Newer');
	const rowFor = (...history: ReturnType<typeof entry>[]) =>
		historyRows(history, heads).find((row) => row.entry.id === 1)!;

	it('holds while the history is as it was, and fails when a newer entry arrived or the row is gone', () => {
		const confirmed = rowFor(newer, older);
		expect(sameChain(rowFor(newer, older), confirmed)).toBe(true);
		expect(sameChain(rowFor(entry(3, 'Arrived'), newer, older), confirmed)).toBe(false);
		expect(sameChain(undefined, confirmed)).toBe(false);
	});
});
