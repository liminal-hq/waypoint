// The operations plugin as the queue UI uses it: the queue's commands, its events, progress and the undo history
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	Clipboard,
	ClipboardMode,
	ConflictPolicy,
	Decision,
	JobId,
	JobProgress,
	JobRequest,
	JournalEntrySummary,
	JournalId,
	Location,
	OpsCommandError,
	OpsEvent,
	OpsSettings,
	OpsSnapshot,
	PlanPreview,
	RecoveryReport,
	Resolution,
} from '@liminal-hq/waypoint-plugin-ops';
import type { Unsubscribe } from './vfsClient';

export type {
	Clipboard,
	ClipboardMode,
	ConflictPolicy,
	Decision,
	JobId,
	JobProgress,
	JobRequest,
	JournalEntrySummary,
	JournalId,
	Location,
	OpsCommandError,
	OpsEvent,
	OpsSettings,
	OpsSnapshot,
	PlanPreview,
	RecoveryReport,
	Resolution,
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
	resolve(job: JobId, decisions: Resolution[], applyToAll?: ConflictPolicy): Promise<void>;
	resolveError(job: JobId, decision: Decision): Promise<void>;
	undo(entry?: JournalId): Promise<JobId>;
	redo(entry?: JournalId): Promise<JobId>;
	journalSummaries(): Promise<JournalEntrySummary[]>;
	/** The unfinished jobs that touch `location`, which the close guard warns about. */
	jobsTargeting(location: Location): Promise<JobId[]>;

	/**
	 * Hears the progress of the running jobs on this window, at the rate the queue's gate allows.
	 * Progress is not an event: only a window that subscribes receives it. Resolves to the function
	 * that stops listening.
	 */
	subscribeProgress(listener: (progress: JobProgress) => void): Promise<() => void>;

	getClipboard(): Promise<Clipboard>;
	setClipboard(mode: ClipboardMode, items: Location[]): Promise<Clipboard>;
	getSettings(): Promise<OpsSettings>;
	setSettings(settings: OpsSettings): Promise<OpsSettings>;
	/** What start-up recovery found, once; `null` when there was nothing to tell and after the first call. */
	takeRecoveryReport(): Promise<RecoveryReport | null>;

	/** Every change to the queue and the undo history, broadcast to every window. */
	onEvent(listener: (event: OpsEvent) => void): Unsubscribe;
	onClipboard(listener: (clipboard: Clipboard) => void): Unsubscribe;
	onRecovered(listener: (report: RecoveryReport) => void): Unsubscribe;
}
