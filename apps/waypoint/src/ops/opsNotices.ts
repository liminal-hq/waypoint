// The toasts the queue makes: Undo after a job that can be undone, and what start-up recovery found
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ResumableRecord } from '@liminal-hq/waypoint-protocol/generated/ResumableRecord';
import { showNotice, type NoticeAction, type NoticeExtras } from '../app/notices';
import { t, tf } from '../i18n/messages';
import type { OpsClient, OpsCommandError, RecoveryReport } from '../services/opsClient';
import type { ConfirmSpec } from './fileCommands';
import { discardInterrupted, resumeInterrupted } from './interrupted';
import { errorText, jobDoneText } from './jobText';
import type { OpsHandle } from './opsStore';

type Show = (text: string, action?: NoticeAction) => unknown;
type ShowWith = (text: string, action?: NoticeAction, extras?: NoticeExtras) => unknown;

/** The words for a rejected command: the engine's own typed reason when it gave one, its message otherwise. */
export function commandErrorText(error: unknown): string {
	const failure = error as Partial<OpsCommandError> | null;
	if (failure?.error) {
		return failure.error.kind === 'undoUnavailable'
			? t('ops.error.undoUnavailable')
			: errorText(failure.error);
	}
	return typeof failure?.message === 'string' ? failure.message : String(error);
}

/**
 * Undoes the newest applied entry (or `entry`, and only that one) and says why when it cannot. The
 * Edit menu, the shortcut and the palette call this; the undo itself is a job in the queue.
 */
export async function runUndo(handle: OpsHandle, show: Show = showNotice, entry?: number) {
	try {
		return await handle.undo(entry);
	} catch (error) {
		// An entry that was named and cannot be undone is not "nothing to undo": it was already
		// undone or has left the history, and the newest entry is not undone in its place.
		const gone =
			entry !== undefined &&
			(error as Partial<OpsCommandError> | null)?.error?.kind === 'undoUnavailable';
		show(
			tf('ops.undo.failed', {
				reason: gone ? t('ops.undo.entryGone') : commandErrorText(error),
			}),
		);
		return null;
	}
}

/**
 * Undoes what one job did, by the journal entry it made: not the newest entry, which another job
 * (in this window or another) may have made since.
 */
export async function undoJob(handle: OpsHandle, show: Show, job: number) {
	let entry: number | null = null;
	try {
		entry = await handle.client.journalEntryOf(job);
	} catch (error) {
		console.warn('could not look up the journal entry of a job', error);
	}
	if (entry === null) {
		show(tf('ops.undo.failed', { reason: t('ops.undo.entryGone') }));
		return null;
	}
	return runUndo(handle, show, entry);
}

export async function runRedo(handle: OpsHandle, show: Show = showNotice, entry?: number) {
	try {
		return await handle.redo(entry);
	} catch (error) {
		show(tf('ops.redo.failed', { reason: commandErrorText(error) }));
		return null;
	}
}

export interface UndoNoticeOptions {
	/** The window's label: only jobs it started get a toast. */
	windowLabel: string;
	show?: Show;
}

/**
 * Shows "Copied 3 items" with an Undo button when a job this window started ends and the journal
 * holds an entry for it (the job's `undoable`, which the plugin sets once the entry is committed).
 * A refused undo or redo job this window started shows why. Jobs already finished when it starts,
 * and those that end in any other way, get none. Returns what stops it.
 */
export function startUndoNotices(handle: OpsHandle, options: UndoNoticeOptions): () => void {
	const show = options.show ?? showNotice;
	const noticed = new Set<number>();
	let seeded = false;
	const process = () => {
		const { snapshot } = handle.store.getState();
		if (!snapshot) return;
		for (const job of snapshot.jobs) {
			if (noticed.has(job.id)) continue;
			if (!seeded) {
				noticed.add(job.id);
				continue;
			}
			const kind = job.kind.kind;
			if (kind === 'undo' || kind === 'redo') {
				if (job.state.state === 'done') noticed.add(job.id);
				else if (job.state.state === 'failed') {
					// A refused undo (the files changed since) says why, in plain words, and nowhere else would.
					noticed.add(job.id);
					if (job.originWindow === options.windowLabel) {
						show(
							tf(kind === 'undo' ? 'ops.undo.failed' : 'ops.redo.failed', {
								reason: errorText(job.state.error),
							}),
						);
					}
				}
				continue;
			}
			if (job.state.state === 'done' && job.undoable) {
				noticed.add(job.id);
				if (job.originWindow === options.windowLabel) {
					show(jobDoneText(job), {
						label: t('notice.undo'),
						run: () => void undoJob(handle, show, job.id),
					});
				}
			}
		}
		seeded = true;
	};
	const stop = handle.store.subscribe(process);
	process();
	return stop;
}

/** The sentence for what recovery found, or `null` when no job was interrupted. */
export function recoveryText(report: RecoveryReport): string | null {
	const [first] = report.interrupted;
	if (!first) return null;
	return report.interrupted.length === 1
		? tf('ops.recovery.one', { label: first.label })
		: tf('ops.recovery.other', { count: report.interrupted.length, label: first.label });
}

/**
 * Offers each transfer a lost connection stopped (D165), one notice after another: "A transfer
 * stopped when its connection was lost: … (1 of 2)" with Resume and Discard…. The next comes when
 * the one before is acted on or goes; the Operations list offers them all as well.
 */
export function offerInterrupted(
	client: Pick<OpsClient, 'resumeInterrupted' | 'discardInterrupted'>,
	records: readonly ResumableRecord[],
	show: ShowWith,
	confirm: (spec: ConfirmSpec) => Promise<boolean>,
	at = 0,
): boolean {
	const record = records[at];
	if (!record) return false;
	const next = () => void offerInterrupted(client, records, show, confirm, at + 1);
	const text =
		records.length === 1
			? tf('ops.recovery.resumable', { label: record.label })
			: tf('ops.recovery.resumableOf', {
					label: record.label,
					n: at + 1,
					count: records.length,
				});
	show(
		text,
		{
			label: t('ops.recovery.resume'),
			run: () => void resumeInterrupted(client, record, (message) => show(message)),
		},
		{
			more: [
				{
					label: t('ops.interrupted.discardEllipsis'),
					run: () => void discardInterrupted(client, record, confirm, (message) => show(message)),
				},
			],
			onClose: next,
		},
	);
	return true;
}

/**
 * Asks Rust for the recovery report (it hands it over once, to whichever window asks first) and
 * shows "An operation was interrupted: …" when there is one, or, for transfers a lost connection
 * stopped, the offer to resume or discard each. The Main window calls it as it starts.
 */
export async function showRecoveryNotice(
	client: OpsClient,
	show: ShowWith = showNotice,
	confirm: (spec: ConfirmSpec) => Promise<boolean> = async () => false,
): Promise<void> {
	try {
		const report = await client.takeRecoveryReport();
		if (!report) return;
		if (offerInterrupted(client, report.resumable ?? [], show, confirm)) return;
		const text = recoveryText(report);
		if (text) show(text);
	} catch (error) {
		console.warn('could not read the recovery report', error);
	}
}
