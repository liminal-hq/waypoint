// The undo history as palette rows: one per entry, and what choosing an older one undoes or redoes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JournalEntrySummary } from '@liminal-hq/waypoint-protocol/generated/JournalEntrySummary';
import { formatModified } from '../browse/format';
import { t, tf } from '../i18n/messages';
import type { ConfirmSpec, HistoryStepOutcome } from '../ops/fileCommands';
import type { HourCycle } from '../services/timeFormatClient';

export type HistoryKind = 'undo' | 'redo';

/** A history entry the palette offers, with the chain choosing it would run. */
export interface HistoryRow {
	/** `history:undo:12`: stable, so the active row survives a refresh of the list. */
	key: string;
	kind: HistoryKind;
	/** The entry the person chose. */
	entry: JournalEntrySummary;
	/** What is applied, in the order it is applied: for an undo the newest first, for a redo the oldest first, ending with `entry`. */
	steps: JournalEntrySummary[];
	/** "Undo: Move 3 items to Trash", or "Undo 3 changes back to: …" when more than the entry itself goes. */
	label: string;
	/** An undo of this entry stopped part way before; undoing it finishes the rest. */
	partly: boolean;
}

export const historyRowKey = (kind: HistoryKind, entry: number) => `history:${kind}:${entry}`;

/** Reads a row's key back, or `null` for any other. */
export function parseHistoryRowKey(key: string): { kind: HistoryKind; entry: number } | null {
	const match = /^history:(undo|redo):(\d+)$/.exec(key);
	return match ? { kind: match[1] as HistoryKind, entry: Number(match[2]) } : null;
}

/**
 * The rows for the history, `history` being newest first as the journal lists it. An applied entry
 * is offered as an undo and an undone one as a redo, except the two the registry's own Undo and
 * Redo act on (`undoHead`, `redoHead`), which are those commands and carry the keys.
 *
 * Undoing an entry also undoes every applied entry newer than it, newest first, because a newer
 * change may rest on an older one. Redoing is the mirror: every undone entry older than the chosen
 * one is redone first, oldest first, because a newer change may rest on an older one there too.
 */
export function historyRows(
	history: readonly JournalEntrySummary[],
	heads: { undoHead: number | null; redoHead: number | null },
): HistoryRow[] {
	const rows: HistoryRow[] = [];
	history.forEach((entry, index) => {
		if (entry.undoable && entry.id !== heads.undoHead) {
			const steps = history.slice(0, index + 1).filter((candidate) => candidate.undoable);
			rows.push(row('undo', entry, steps));
		}
	});
	// Oldest first: the redo chain replays forward in time and each row's steps are a prefix of it.
	const redoable = [...history].reverse().filter((entry) => entry.redoable);
	redoable.forEach((entry, index) => {
		if (entry.id !== heads.redoHead) rows.push(row('redo', entry, redoable.slice(0, index + 1)));
	});
	return rows;
}

function row(
	kind: HistoryKind,
	entry: JournalEntrySummary,
	steps: JournalEntrySummary[],
): HistoryRow {
	const many = steps.length > 1;
	const key = `history.row.${kind}${many ? 'Many' : ''}` as const;
	return {
		key: historyRowKey(kind, entry.id),
		kind,
		entry,
		steps,
		label: tf(key, { label: entry.label, count: steps.length }),
		partly: kind === 'undo' && entry.partlyUndone,
	};
}

/**
 * Whether the chain the person confirmed is still what the history says: the same row with the
 * same entries in the same order. A job that finished meanwhile adds a newest entry the chain
 * does not hold, and undoing the older ones beneath it would leave a change resting on nothing.
 */
export function sameChain(current: HistoryRow | undefined, confirmed: HistoryRow): boolean {
	return (
		current !== undefined &&
		current.key === confirmed.key &&
		current.steps.length === confirmed.steps.length &&
		current.steps.every((step, index) => step.id === confirmed.steps[index]!.id)
	);
}

/** Whether choosing the row asks first: more than one change goes, or an undo that stopped part way is finished. */
export function needsConfirmation(row: HistoryRow): boolean {
	return row.steps.length > 1 || row.steps.some((step) => step.partlyUndone && row.kind === 'undo');
}

/** The question for a row: what goes, in the order it goes. Focus starts on Cancel (the dialog's default for a danger button). */
export function confirmSpec(row: HistoryRow): ConfirmSpec {
	const partlyAmong = row.kind === 'undo' && row.steps.some((step) => step.partlyUndone);
	const single = row.steps.length === 1;
	return {
		title:
			single && partlyAmong
				? t('history.confirm.partly.title')
				: tf(`history.confirm.${row.kind}.title`, { count: row.steps.length }),
		message:
			single && partlyAmong
				? t('history.confirm.partly.message')
				: t(`history.confirm.${row.kind}.message`),
		items: row.steps.map((step) => step.label),
		...(!single && partlyAmong ? { note: t('history.confirm.partly.note') } : {}),
		confirmLabel: t(`history.confirm.${row.kind}.confirm`),
		danger: true,
	};
}

/** What a chain did: how far it got, and why it stopped when it did not finish. */
export interface HistoryReport {
	done: JournalEntrySummary[];
	/** The entry that was refused, or `null` when every step went through. */
	stoppedAt: JournalEntrySummary | null;
	reason: string | null;
	/** The sentence to show and read out. */
	text: string;
}

export interface HistoryDeps {
	step(kind: HistoryKind, entry: number): Promise<HistoryStepOutcome>;
}

/**
 * Runs a row's chain one entry at a time, each as its own job and each waited for before the next,
 * and stops at the first the engine refuses. The report says what was applied and why it stopped,
 * in the engine's own plain words.
 */
export async function applyHistory(row: HistoryRow, deps: HistoryDeps): Promise<HistoryReport> {
	const done: JournalEntrySummary[] = [];
	for (const step of row.steps) {
		const outcome = await deps.step(row.kind, step.id);
		if (!outcome.ok) return stopped(row, done, step, outcome.reason);
		done.push(step);
	}
	const text =
		row.steps.length === 1
			? tf(`history.done.${row.kind}One`, { label: row.entry.label })
			: tf(`history.done.${row.kind}Many`, { count: row.steps.length, label: row.entry.label });
	return { done, stoppedAt: null, reason: null, text };
}

function stopped(
	row: HistoryRow,
	done: JournalEntrySummary[],
	at: JournalEntrySummary,
	reason: string,
): HistoryReport {
	const text =
		done.length === 0
			? tf(`history.stopped.first.${row.kind}`, { label: at.label, reason })
			: tf(`history.stopped.${row.kind}`, {
					done: done.length,
					total: row.steps.length,
					label: at.label,
					reason,
				});
	return { done, stoppedAt: at, reason, text };
}

/**
 * When an entry happened, for a row: "2 minutes ago" or "yesterday" up to a week, then the date
 * and time in the system's 12 or 24-hour setting.
 */
export function relativeTime(
	atMs: number,
	now: number = Date.now(),
	locale?: string,
	hourCycle?: HourCycle,
): string {
	const seconds = Math.round((atMs - now) / 1000);
	const away = Math.abs(seconds);
	const format = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
	if (away < 45) return format.format(0, 'second');
	if (away < 45 * 60) return format.format(Math.round(seconds / 60), 'minute');
	if (away < 22 * 3600) return format.format(Math.round(seconds / 3600), 'hour');
	if (away < 7 * 86400) return format.format(Math.round(seconds / 86400), 'day');
	return formatModified(atMs, locale, hourCycle);
}

/** The exact time, for a tooltip beside the relative one. */
export function exactTime(atMs: number, locale?: string, hourCycle?: HourCycle): string {
	return formatModified(atMs, locale, hourCycle);
}
