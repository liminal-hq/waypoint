// Exposes typed guest-side wrappers for the waypoint-ops plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Channel, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { BatchPreview } from '@liminal-hq/waypoint-protocol/generated/BatchPreview';
import type { Clipboard } from '@liminal-hq/waypoint-protocol/generated/Clipboard';
import type { ClipboardSource } from '@liminal-hq/waypoint-protocol/generated/ClipboardSource';
import type { ClipboardMode } from '@liminal-hq/waypoint-protocol/generated/ClipboardMode';
import type { ConflictPolicy } from '@liminal-hq/waypoint-protocol/generated/ConflictPolicy';
import type { Decision } from '@liminal-hq/waypoint-protocol/generated/Decision';
import type { JobId } from '@liminal-hq/waypoint-protocol/generated/JobId';
import type { JobJournal } from '@liminal-hq/waypoint-protocol/generated/JobJournal';
import type { JobProgress } from '@liminal-hq/waypoint-protocol/generated/JobProgress';
import type { JobRequest } from '@liminal-hq/waypoint-protocol/generated/JobRequest';
import type { JournalEntrySummary } from '@liminal-hq/waypoint-protocol/generated/JournalEntrySummary';
import type { JournalId } from '@liminal-hq/waypoint-protocol/generated/JournalId';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import type { OpsEvent } from '@liminal-hq/waypoint-protocol/generated/OpsEvent';
import type { OpsSettings } from '@liminal-hq/waypoint-protocol/generated/OpsSettings';
import type { OpsSnapshot } from '@liminal-hq/waypoint-protocol/generated/OpsSnapshot';
import type { PlanNote } from '@liminal-hq/waypoint-protocol/generated/PlanNote';
import type { PlanPreview } from '@liminal-hq/waypoint-protocol/generated/PlanPreview';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import type { RecoveryReport } from '@liminal-hq/waypoint-protocol/generated/RecoveryReport';
import type { RenameRule } from '@liminal-hq/waypoint-protocol/generated/RenameRule';
import type { RenameSpec } from '@liminal-hq/waypoint-protocol/generated/RenameSpec';
import type { Resolution } from '@liminal-hq/waypoint-protocol/generated/Resolution';
import type { PreviewRow } from '@liminal-hq/waypoint-protocol/generated/PreviewRow';
import type { Problem } from '@liminal-hq/waypoint-protocol/generated/Problem';
import type { RuleError } from '@liminal-hq/waypoint-protocol/generated/RuleError';
import type { SelectionSpec } from '@liminal-hq/waypoint-protocol/generated/SelectionSpec';

export type {
	BatchPreview,
	Clipboard,
	ClipboardMode,
	ClipboardSource,
	ConflictPolicy,
	Decision,
	JobId,
	JobJournal,
	JobProgress,
	JobRequest,
	JournalEntrySummary,
	JournalId,
	ListingHandle,
	Location,
	OpsError,
	OpsEvent,
	OpsSettings,
	OpsSnapshot,
	PlanNote,
	PlanPreview,
	PluginStatus,
	PreviewRow,
	Problem,
	RecoveryReport,
	RenameRule,
	RenameSpec,
	Resolution,
	RuleError,
	SelectionSpec,
};

const PREFIX = 'plugin:waypoint-ops|';

/** Every change to the queue or the undo history, broadcast to every window. */
export const OPS_EVENT = 'waypoint-ops://event';
/** The shared clipboard changed, broadcast to every window. */
export const CLIPBOARD_EVENT = 'waypoint-ops://clipboard';
/** A job recorded its journal entry (a `JobJournal`), broadcast to every window. */
export const JOB_JOURNAL_EVENT = 'waypoint-ops://job-journal';
/** What the last run left interrupted, sent once at start-up. */
export const RECOVERED_EVENT = 'waypoint-ops://recovered';

/**
 * What a rejected command carries. `ops` is the engine's own typed refusal or failure (in `error`),
 * `queue` a change the queue refused (an unknown job, or a state that cannot do that), `invalid` a
 * setting out of range and `storage` settings that could not be saved.
 */
export interface OpsCommandError {
	kind: 'internal' | 'queue' | 'ops' | 'invalid' | 'storage';
	message: string;
	error?: OpsError;
}

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports whether the operations plugin works and which features it offers. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** The queue and the undo history at their current revisions. */
export function getSnapshot(): Promise<OpsSnapshot> {
	return cmd<OpsSnapshot>('get_snapshot');
}

/**
 * What a request would do, found without queueing it: the counts, the clashes and whether every
 * source is on the destination's volume (a drag moves on one volume and copies across).
 */
export function plan(request: JobRequest): Promise<PlanPreview> {
	return cmd<PlanPreview>('plan', { request });
}

/**
 * What a batch rename would do, found without queueing it: `request` is the `batchRename` request
 * that `submit` takes (its sources and its `rename` rules). The answer has a row for each entry
 * with its new name and what is wrong with it, and `nowMs`, the time "today" meant, which goes back
 * in the submitted request's `rename.nowMs` so the job writes the names that were previewed.
 */
export function previewBatchRename(request: JobRequest): Promise<BatchPreview> {
	return cmd<BatchPreview>('preview_batch_rename', { request });
}

/** Puts a request on the queue and returns the job's id. */
export function submit(request: JobRequest): Promise<JobId> {
	return cmd<JobId>('submit', { request });
}

export function pause(job: JobId): Promise<void> {
	return cmd<void>('pause', { job });
}

export function resume(job: JobId): Promise<void> {
	return cmd<void>('resume', { job });
}

export function cancel(job: JobId): Promise<void> {
	return cmd<void>('cancel', { job });
}

/** Makes a new job from a failed or cancelled one's request and returns its id. */
export function retry(job: JobId): Promise<JobId> {
	return cmd<JobId>('retry', { job });
}

/** Removes a finished job from the list. */
export function dismiss(job: JobId): Promise<void> {
	return cmd<void>('dismiss', { job });
}

export function dismissFinished(): Promise<void> {
	return cmd<void>('dismiss_finished');
}

/** Moves a queued job to `to` among the queued jobs (0 runs next). */
export function reorder(job: JobId, to: number): Promise<void> {
	return cmd<void>('reorder', { job, to });
}

/**
 * Answers the conflicts a job waits on: each decision settles one source's clash, and
 * `applyToAll` is the policy for every other clash the job meets.
 */
export function resolve(
	job: JobId,
	decisions: Resolution[],
	applyToAll?: ConflictPolicy,
): Promise<void> {
	return cmd<void>('resolve', { job, decisions, applyToAll: applyToAll ?? null });
}

/** Answers the error a job waits on. */
export function resolveError(job: JobId, decision: Decision): Promise<void> {
	return cmd<void>('resolve_error', { job, decision });
}

/** Undoes an entry of the journal (the newest applied one when omitted) and returns the job's id. */
export function undo(entry?: JournalId): Promise<JobId> {
	return cmd<JobId>('undo', { entry: entry ?? null });
}

/** Redoes an entry of the journal (the one undone most recently when omitted). */
export function redo(entry?: JournalId): Promise<JobId> {
	return cmd<JobId>('redo', { entry: entry ?? null });
}

/** The undo history, newest first. */
export function journalSummaries(): Promise<JournalEntrySummary[]> {
	return cmd<JournalEntrySummary[]>('journal_summaries');
}

/** The journal entry the job made, or `null` while it has none (unfinished, or it changed nothing). */
export function journalEntryOf(job: JobId): Promise<JournalId | null> {
	return cmd<JournalId | null>('journal_entry_of', { job });
}

/**
 * Hears the progress of the running jobs on this window, at the rate the queue's gate allows
 * (every 100 ms and 1 %). Progress is not an event: only a window that subscribes receives it.
 * Returns the function that stops listening: it stops this subscription only, so a stop that runs
 * after the window has subscribed again (a page that mounted twice) leaves the newer one alone.
 */
export async function subscribeProgress(
	onProgress: (progress: JobProgress) => void,
): Promise<() => Promise<void>> {
	const channel = new Channel<JobProgress>();
	channel.onmessage = onProgress;
	const token = await cmd<number>('subscribe_progress', { onProgress: channel });
	return () => cmd<void>('unsubscribe_progress', { token });
}

/**
 * Replaces the shared clipboard; an empty list clears it. Every window is told. `source` says who
 * set it (`app` when omitted; `os` for files adopted from another application's clipboard).
 */
export function setClipboard(
	mode: ClipboardMode,
	items: Location[],
	source?: ClipboardSource,
): Promise<Clipboard> {
	return cmd<Clipboard>('set_clipboard', { mode, items, source: source ?? null });
}

/**
 * Puts the entries a selection covers on the shared clipboard. Rust resolves them from the listing
 * this window opened, so a selection of a hundred thousand files is still a handle and a spec.
 * Rejects (`unsupported`) for a selection of nothing.
 */
export function setClipboardFromSelection(
	handle: ListingHandle,
	spec: SelectionSpec,
	mode: ClipboardMode,
): Promise<Clipboard> {
	return cmd<Clipboard>('set_clipboard_from_selection', { handle, spec, mode });
}

/**
 * The locations a selection covers, resolved by Rust from the listing this window opened, for a
 * drag that leaves the window. Each location's `uri` is the lossless `file://` form. Rejects
 * (`unsupported`) for a selection of nothing.
 */
export function resolveSelection(handle: ListingHandle, spec: SelectionSpec): Promise<Location[]> {
	return cmd<Location[]>('resolve_selection', { handle, spec });
}

export function getClipboard(): Promise<Clipboard> {
	return cmd<Clipboard>('get_clipboard');
}

/** The unfinished jobs that touch `location`, which the close guard warns about. */
export function jobsTargeting(location: Location): Promise<JobId[]> {
	return cmd<JobId[]>('jobs_targeting', { location });
}

export function getSettings(): Promise<OpsSettings> {
	return cmd<OpsSettings>('get_settings');
}

/** Saves new settings, which the next job reads, and returns what is in force. */
export function setSettings(settings: OpsSettings): Promise<OpsSettings> {
	return cmd<OpsSettings>('set_settings', { settings });
}

/**
 * What start-up recovery found, once: the jobs the last run left interrupted and what was cleaned
 * up. `null` when there was nothing to tell, and after the first call.
 */
export function takeRecoveryReport(): Promise<RecoveryReport | null> {
	return cmd<RecoveryReport | null>('take_recovery_report');
}

/**
 * Follows every change to the queue and the undo history. Read `getSnapshot()` first and apply the
 * events with a higher revision (a `journalChanged` is judged against the journal's own revision);
 * the revisions have gaps, since progress ticks use them up.
 */
export function onOpsEvent(listener: (event: OpsEvent) => void): Promise<UnlistenFn> {
	return listen<OpsEvent>(OPS_EVENT, (e) => listener(e.payload));
}

/** Follows the shared clipboard. */
export function onClipboardChanged(listener: (clipboard: Clipboard) => void): Promise<UnlistenFn> {
	return listen<Clipboard>(CLIPBOARD_EVENT, (e) => listener(e.payload));
}

/** Hears the recovery report if a window is listening when it is sent at start-up. */
export function onRecovered(listener: (report: RecoveryReport) => void): Promise<UnlistenFn> {
	return listen<RecoveryReport>(RECOVERED_EVENT, (e) => listener(e.payload));
}
