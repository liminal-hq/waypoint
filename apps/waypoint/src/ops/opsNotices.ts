// The toasts the queue makes: Undo after a job that can be undone, and what start-up recovery found
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { showNotice, type NoticeAction } from '../app/notices';
import { t, tf } from '../i18n/messages';
import type { OpsClient, OpsCommandError, RecoveryReport } from '../services/opsClient';
import { errorText, jobDoneText } from './jobText';
import type { OpsHandle } from './opsStore';

type Show = (text: string, action?: NoticeAction) => unknown;

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
 * Undoes the newest applied entry (or `entry`) and says why when it cannot. The Edit menu, the
 * shortcut and the palette call this; the undo itself is a job in the queue.
 */
export async function runUndo(handle: OpsHandle, show: Show = showNotice, entry?: number) {
	try {
		return await handle.undo(entry);
	} catch (error) {
		show(tf('ops.undo.failed', { reason: commandErrorText(error) }));
		return null;
	}
}

export async function runRedo(handle: OpsHandle, show: Show = showNotice, entry?: number) {
	try {
		return await handle.redo(entry);
	} catch (error) {
		show(tf('ops.undo.failed', { reason: commandErrorText(error) }));
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
 * Jobs already finished when it starts, and those that end in any other way, get none. Returns what
 * stops it.
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
				continue;
			}
			if (job.state.state === 'done' && job.undoable) {
				noticed.add(job.id);
				if (job.originWindow === options.windowLabel) {
					show(jobDoneText(job), {
						label: t('notice.undo'),
						run: () => void runUndo(handle, show),
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
 * Asks Rust for the recovery report (it hands it over once, to whichever window asks first) and
 * shows "An operation was interrupted: …" when there is one. The Main window calls it as it starts.
 */
export async function showRecoveryNotice(
	client: OpsClient,
	show: Show = showNotice,
): Promise<void> {
	try {
		const report = await client.takeRecoveryReport();
		const text = report ? recoveryText(report) : null;
		if (text) show(text);
	} catch (error) {
		console.warn('could not read the recovery report', error);
	}
}
