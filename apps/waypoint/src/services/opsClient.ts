// The operations plugin as the queue UI uses it: the queue's commands, its events, progress and the undo history
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	Clipboard,
	ClipboardMode,
	ClipboardSource,
	ConflictPolicy,
	ConflictPreview,
	Decision,
	DiffLine,
	JobId,
	JobPriority,
	JobProgress,
	JobRequest,
	JournalEntrySummary,
	JournalId,
	ListingHandle,
	Location,
	OpsCommandError,
	OpsEvent,
	OpsSettings,
	OpsSnapshot,
	PlanPreview,
	RecoveryReport,
	Resolution,
	SelectionSpec,
} from '@liminal-hq/waypoint-plugin-ops';
import type { Unsubscribe } from './vfsClient';

export type {
	Clipboard,
	ClipboardMode,
	ClipboardSource,
	ConflictPolicy,
	ConflictPreview,
	Decision,
	DiffLine,
	JobId,
	JobPriority,
	JobProgress,
	JobRequest,
	JournalEntrySummary,
	JournalId,
	ListingHandle,
	Location,
	OpsCommandError,
	OpsEvent,
	OpsSettings,
	OpsSnapshot,
	PlanPreview,
	RecoveryReport,
	Resolution,
	SelectionSpec,
};

/**
 * Everything the queue UI asks of the operations plugin. Rust owns the queue: each command asks for
 * a change and the answer arrives as an event, never as a return value to apply. `FakeOpsClient`
 * keeps the same state machine in memory, so every component is testable without Tauri.
 */
export interface OpsClient {
	/** The queue and the undo history at their current revisions. */
	snapshot(): Promise<OpsSnapshot>;
	plan(request: JobRequest): Promise<PlanPreview>;
	submit(request: JobRequest): Promise<JobId>;
	pause(job: JobId): Promise<void>;
	resume(job: JobId): Promise<void>;
	cancel(job: JobId): Promise<void>;
	/** Makes a new job from a failed or cancelled one's request. */
	retry(job: JobId): Promise<JobId>;
	dismiss(job: JobId): Promise<void>;
	dismissFinished(): Promise<void>;
	/** Moves a queued job to `to` among the queued jobs (0 runs next). */
	reorder(job: JobId, to: number): Promise<void>;
	/**
	 * Sets a queued or running job's own speed limit in bytes a second (`null` for none) and its
	 * priority (`null` is normal). A running copy obeys the new limit at once.
	 */
	setJobLimits(job: JobId, speedLimit: number | null, priority: JobPriority | null): Promise<void>;
	resolve(job: JobId, decisions: Resolution[], applyToAll?: ConflictPolicy): Promise<void>;
	/**
	 * The two files of one clash a waiting job holds (`item` is the clash's source): sizes, times and
	 * a line diff of small text files. Optional: a client without it leaves the dialog as it was.
	 */
	conflictPreview?(job: JobId, item: Location): Promise<ConflictPreview>;
	resolveError(job: JobId, decision: Decision): Promise<void>;
	undo(entry?: JournalId): Promise<JobId>;
	redo(entry?: JournalId): Promise<JobId>;
	journalSummaries(): Promise<JournalEntrySummary[]>;
	/** The journal entry the job made, or `null` while it has none (unfinished, or it changed nothing). */
	journalEntryOf(job: JobId): Promise<JournalId | null>;
	/** The unfinished jobs that touch `location`, which the close guard warns about. */
	jobsTargeting(location: Location): Promise<JobId[]>;

	/**
	 * Hears the progress of the running jobs on this window, at the rate the queue's gate allows.
	 * Progress is not an event: only a window that subscribes receives it. Resolves to the function
	 * that stops listening, which ends this subscription only: a stop that runs after the window has
	 * subscribed again leaves the newer one alone.
	 */
	subscribeProgress(listener: (progress: JobProgress) => void): Promise<() => void>;

	getClipboard(): Promise<Clipboard>;
	/** Replaces the shared clipboard; an empty list clears it. `source` is `app` when omitted. */
	setClipboard(
		mode: ClipboardMode,
		items: Location[],
		source?: ClipboardSource,
	): Promise<Clipboard>;
	/** Puts what a selection of a listing this window opened covers on the clipboard; Rust resolves the locations. */
	setClipboardFromSelection(
		handle: ListingHandle,
		spec: SelectionSpec,
		mode: ClipboardMode,
	): Promise<Clipboard>;
	/**
	 * The locations a selection of a listing this window opened covers, resolved by Rust, for a
	 * drag of the selection out of the window. The clipboard is left alone; a selection of nothing is refused.
	 */
	resolveSelection(handle: ListingHandle, spec: SelectionSpec): Promise<Location[]>;
	getSettings(): Promise<OpsSettings>;
	setSettings(settings: OpsSettings): Promise<OpsSettings>;
	/** What start-up recovery found, once; `null` when there was nothing to tell and after the first call. */
	takeRecoveryReport(): Promise<RecoveryReport | null>;

	/** Every change to the queue and the undo history, broadcast to every window. */
	onEvent(listener: (event: OpsEvent) => void): Unsubscribe;
	onClipboard(listener: (clipboard: Clipboard) => void): Unsubscribe;
	onRecovered(listener: (report: RecoveryReport) => void): Unsubscribe;
	/**
	 * A notification's "Show" asked this window to show the question a job waits on, as the id of
	 * the job. A window that does not hold the job's question ignores it.
	 */
	onShowJob(listener: (job: JobId) => void): Unsubscribe;
}
