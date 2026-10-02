// An in-memory OpsClient with the queue's state machine, scripted by tests and the demo
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Conflict } from '@liminal-hq/waypoint-protocol/generated/Conflict';
import type { ConflictPreview } from '@liminal-hq/waypoint-protocol/generated/ConflictPreview';
import type { Counts } from '@liminal-hq/waypoint-protocol/generated/Counts';
import type { JobKind } from '@liminal-hq/waypoint-protocol/generated/JobKind';
import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { JobState } from '@liminal-hq/waypoint-protocol/generated/JobState';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import type { Progress } from '@liminal-hq/waypoint-protocol/generated/Progress';
import type {
	Clipboard,
	ClipboardMode,
	ClipboardSource,
	ConflictPolicy,
	Decision,
	JobId,
	JobProgress,
	JobRequest,
	JournalEntrySummary,
	JournalId,
	ListingHandle,
	Location,
	OpsClient,
	OpsEvent,
	OpsSettings,
	OpsSnapshot,
	PlanPreview,
	RecoveryReport,
	Resolution,
	SelectionSpec,
} from './opsClient';

/**
 * The legal moves of a job's state, by state name: a mirror of the table at the top of
 * `crates/waypoint-ops/src/queue.rs`, which a test compares against that comment. Anything else
 * is refused and changes nothing.
 */
export const LEGAL_TRANSITIONS: Readonly<Record<string, readonly string[]>> = {
	planning: ['queued', 'failed', 'cancelled'],
	queued: ['running', 'cancelled'],
	running: ['paused', 'waiting', 'done', 'failed', 'cancelling'],
	paused: ['running', 'cancelling'],
	waiting: ['running', 'cancelling'],
	cancelling: ['cancelled', 'done', 'failed'],
	cancelled: [],
	done: [],
	failed: [],
};

export function isLegalTransition(from: JobState['state'], to: JobState['state']): boolean {
	return LEGAL_TRANSITIONS[from]?.includes(to) ?? false;
}

const FINISHED: ReadonlySet<string> = new Set(['done', 'failed', 'cancelled']);

export interface FakeOpsOptions {
	/** How many jobs hold a worker slot at once; a start beyond it is refused. */
	concurrency?: number;
	/** Plans each submitted job at once (`true`, the default) or waits for `planned()`. */
	autoPlan?: boolean;
	/** Starts queued jobs as slots free up (`true`) or waits for `start()` (the default). */
	autoStart?: boolean;
	/** The wall clock, in milliseconds. */
	now?: () => number;
	/**
	 * What a selection of a listing covers, which Rust's resolver works out in the app. Without it
	 * putting a selection on the clipboard is refused.
	 */
	resolveSelection?: (
		handle: ListingHandle,
		spec: SelectionSpec,
	) => Location[] | Promise<Location[]>;
	/** What the planner would find for a request. */
	totals?: (request: JobRequest) => { items: number; bytes: number };
}

/** What a rejected command carries, as `OpsCommandError` does. */
function refusal(message: string, kind: 'queue' | 'ops' = 'queue', error?: OpsError) {
	return { kind, message, ...(error ? { error } : {}) };
}

interface Entry {
	snapshot: JobSnapshot;
	request: JobRequest;
	resolved: Resolution[];
}

interface JournalRow {
	id: JournalId;
	label: string;
	atMs: number;
	applied: boolean;
	undoneAt: number;
}

function basename(display: string): string {
	const parts = display.split('/').filter(Boolean);
	return parts[parts.length - 1] ?? display;
}

/**
 * The queue in memory, with the same state machine as the Rust store, for tests and the demo.
 * The commands behave as the plugin's do (a refused one rejects and changes nothing); the script
 * methods stand in for the worker pool and the user's side of a wait: `start`, `tick`, `done`,
 * `fail`, `askConflicts`, `askError` and `settleCancel`.
 */
export class FakeOpsClient implements OpsClient {
	private readonly entries: Entry[] = [];
	private revision = 0;
	private journalRevision = 0;
	private nextJob = 1;
	private nextJournal = 1;
	private readonly journal: JournalRow[] = [];
	private readonly entryOfJob = new Map<JobId, JournalId>();
	private readonly eventListeners = new Set<(event: OpsEvent) => void>();
	private readonly progressListeners = new Set<(progress: JobProgress) => void>();
	private readonly clipboardListeners = new Set<(clipboard: Clipboard) => void>();
	private readonly recoveredListeners = new Set<(report: RecoveryReport) => void>();
	private clipboard: Clipboard = { mode: 'copy', items: [], source: 'app', revision: 0 };
	private settings: OpsSettings = {
		concurrency: 2,
		verifyAfterCopy: false,
		verifyAlgorithm: 'blake3',
		confirmTrash: false,
		undoDepth: 50,
		trashExpiryDays: null,
	};
	private recovery: RecoveryReport | null = null;
	private mute = 0;
	private readonly options: Required<Omit<FakeOpsOptions, 'totals' | 'resolveSelection'>> &
		Pick<FakeOpsOptions, 'totals' | 'resolveSelection'>;

	/** Every command the UI sent, in order, for assertions: `[name, ...arguments]`. */
	readonly calls: unknown[][] = [];

	constructor(options: FakeOpsOptions = {}) {
		this.options = {
			concurrency: options.concurrency ?? 2,
			autoPlan: options.autoPlan ?? true,
			autoStart: options.autoStart ?? false,
			now: options.now ?? (() => Date.now()),
			totals: options.totals,
			resolveSelection: options.resolveSelection,
		};
	}

	// --- Reading -------------------------------------------------------------------------------

	async snapshot(): Promise<OpsSnapshot> {
		return this.currentSnapshot();
	}

	private currentSnapshot(): OpsSnapshot {
		const applied = this.journal.filter((row) => row.applied);
		const undone = this.journal.filter((row) => !row.applied);
		const newestApplied = applied[applied.length - 1];
		const lastUndone = [...undone].sort((a, b) => b.undoneAt - a.undoneAt)[0];
		return {
			revision: this.revision,
			jobs: this.entries.map((entry) => structuredClone(entry.snapshot)),
			journal: {
				revision: this.journalRevision,
				undo: newestApplied ? this.summary(newestApplied) : null,
				redo: lastUndone ? this.summary(lastUndone) : null,
			},
		};
	}

	private summary(row: JournalRow): JournalEntrySummary {
		return {
			id: row.id,
			label: row.label,
			atMs: row.atMs,
			undoable: row.applied,
			redoable: !row.applied,
			partlyUndone: false,
		};
	}

	async journalSummaries(): Promise<JournalEntrySummary[]> {
		return [...this.journal].reverse().map((row) => this.summary(row));
	}

	async journalEntryOf(job: JobId): Promise<JournalId | null> {
		this.calls.push(['journalEntryOf', job]);
		return this.entryOfJob.get(job) ?? null;
	}

	async plan(request: JobRequest): Promise<PlanPreview> {
		this.calls.push(['plan', request]);
		const totals = this.totalsFor(request);
		return {
			kind: request.kind,
			sources: this.summarise(request),
			items: totals.items,
			bytes: totals.bytes,
			sameVolume: true,
			conflicts: [],
			notes: [],
		};
	}

	async jobsTargeting(location: Location): Promise<JobId[]> {
		return this.entries
			.filter(
				(entry) =>
					!FINISHED.has(entry.snapshot.state.state) &&
					(entry.snapshot.destination?.uri === location.uri ||
						(entry.request.sources.kind === 'locations' &&
							entry.request.sources.locations.some((l) => l.uri === location.uri))),
			)
			.map((entry) => entry.snapshot.id);
	}

	// --- Commands ------------------------------------------------------------------------------

	async submit(request: JobRequest): Promise<JobId> {
		this.calls.push(['submit', request]);
		const id = this.add(request);
		if (this.options.autoPlan) this.planned(id);
		return id;
	}

	async pause(job: JobId): Promise<void> {
		this.calls.push(['pause', job]);
		this.go(this.find(job), { state: 'paused' }, 'pause');
	}

	async resume(job: JobId): Promise<void> {
		this.calls.push(['resume', job]);
		const entry = this.find(job);
		if (entry.snapshot.state.state !== 'paused') this.illegal(entry, 'resume');
		this.go(entry, { state: 'running' }, 'resume');
	}

	async cancel(job: JobId): Promise<void> {
		this.calls.push(['cancel', job]);
		const entry = this.find(job);
		const state = entry.snapshot.state.state;
		if (state === 'cancelling') return;
		if (state === 'planning' || state === 'queued') {
			this.go(entry, { state: 'cancelled' }, 'cancel');
		} else {
			this.go(entry, { state: 'cancelling' }, 'cancel');
		}
	}

	async retry(job: JobId): Promise<JobId> {
		this.calls.push(['retry', job]);
		const entry = this.find(job);
		const state = entry.snapshot.state.state;
		if (state !== 'failed' && state !== 'cancelled') this.illegal(entry, 'retry');
		const id = this.add(structuredClone(entry.request));
		if (this.options.autoPlan) this.planned(id);
		return id;
	}

	async dismiss(job: JobId): Promise<void> {
		this.calls.push(['dismiss', job]);
		this.remove(this.find(job));
	}

	async dismissFinished(): Promise<void> {
		this.calls.push(['dismissFinished']);
		for (const entry of [...this.entries]) {
			if (FINISHED.has(entry.snapshot.state.state)) this.remove(entry);
		}
	}

	async reorder(job: JobId, to: number): Promise<void> {
		this.calls.push(['reorder', job, to]);
		const entry = this.find(job);
		if (entry.snapshot.state.state !== 'queued') this.illegal(entry, 'reorder');
		const slots: number[] = [];
		this.entries.forEach((e, i) => {
			if (e.snapshot.state.state === 'queued') slots.push(i);
		});
		const queued = slots.map((i) => this.entries[i]!).filter((e) => e !== entry);
		queued.splice(Math.min(to, queued.length), 0, entry);
		slots.forEach((slot, i) => {
			this.entries[slot] = queued[i]!;
		});
		this.revision += 1;
		this.emit({
			kind: 'queueReordered',
			order: this.entries.map((e) => e.snapshot.id),
			revision: this.revision,
		});
	}

	async resolve(job: JobId, decisions: Resolution[], applyToAll?: ConflictPolicy): Promise<void> {
		this.calls.push(['resolve', job, decisions, applyToAll]);
		const entry = this.find(job);
		const state = entry.snapshot.state;
		if (state.state !== 'waiting' || state.reason.kind !== 'conflicts') {
			this.illegal(entry, 'take conflict answers');
			return;
		}
		const answers = applyToAll ? [...decisions, { source: null, policy: applyToAll }] : decisions;
		entry.resolved.push(...answers);
		const policyForAll = answers.some((answer) => answer.source === null);
		const answered = new Set(
			answers.flatMap((answer) => (answer.source ? [answer.source.uri] : [])),
		);
		const remaining = policyForAll
			? []
			: state.reason.conflicts.filter((conflict) => !answered.has(conflict.source.uri));
		if (remaining.length === 0) {
			this.go(entry, { state: 'running' }, 'take an answer');
		} else if (remaining.length < state.reason.conflicts.length) {
			entry.snapshot.state = {
				state: 'waiting',
				reason: { kind: 'conflicts', conflicts: remaining },
			};
			this.changed(entry);
		}
	}

	/**
	 * What `conflictPreview` answers, by the clash's source URI: a preview, or a rejection to
	 * simulate a read that failed. A clash with nothing scripted rejects, as the plugin does for one
	 * the job is not waiting on.
	 */
	readonly previews = new Map<string, ConflictPreview | Error>();
	/** Holds every `conflictPreview` until `release` is called, to test the dialog while it loads. */
	holdPreviews(): { release(): void } {
		let release: () => void = () => undefined;
		this.previewGate = new Promise<void>((resolve) => {
			release = () => {
				this.previewGate = null;
				resolve();
			};
		});
		return { release };
	}
	private previewGate: Promise<void> | null = null;

	async conflictPreview(job: JobId, item: Location): Promise<ConflictPreview> {
		this.calls.push(['conflictPreview', job, item]);
		if (this.previewGate) await this.previewGate;
		const state = this.find(job).snapshot.state;
		if (state.state !== 'waiting' || state.reason.kind !== 'conflicts') {
			throw refusal(`job ${job} is not waiting on conflicts`);
		}
		const scripted = this.previews.get(item.uri);
		if (!scripted) throw refusal(`${item.display} is not a clash of job ${job}`);
		if (scripted instanceof Error) throw scripted;
		return scripted;
	}

	async resolveError(job: JobId, decision: Decision): Promise<void> {
		this.calls.push(['resolveError', job, decision]);
		const entry = this.find(job);
		const state = entry.snapshot.state;
		if (state.state !== 'waiting' || state.reason.kind !== 'error') {
			this.illegal(entry, 'take an answer');
			return;
		}
		this.go(
			entry,
			decision === 'cancel' ? { state: 'cancelling' } : { state: 'running' },
			'take an answer',
		);
	}

	async undo(entry?: JournalId): Promise<JobId> {
		this.calls.push(['undo', entry]);
		const applied = this.journal.filter((row) => row.applied);
		const row =
			entry === undefined ? applied[applied.length - 1] : this.journal.find((r) => r.id === entry);
		if (!row?.applied) {
			throw refusal('there is nothing to undo', 'ops', {
				kind: 'undoUnavailable',
				reason: 'there is nothing to undo',
			});
		}
		return this.submitJournalJob({ kind: 'undo', of: row.id }, row.label);
	}

	async redo(entry?: JournalId): Promise<JobId> {
		this.calls.push(['redo', entry]);
		const undone = this.journal
			.filter((row) => !row.applied)
			.sort((a, b) => b.undoneAt - a.undoneAt);
		const row = entry === undefined ? undone[0] : this.journal.find((r) => r.id === entry);
		if (!row || row.applied) {
			throw refusal('there is nothing to redo', 'ops', {
				kind: 'undoUnavailable',
				reason: 'there is nothing to redo',
			});
		}
		return this.submitJournalJob({ kind: 'redo', of: row.id }, row.label);
	}

	private submitJournalJob(kind: JobKind, label: string): JobId {
		const request = this.requestFor(kind, null);
		const id = this.add(request, label);
		if (this.options.autoPlan) this.planned(id);
		return id;
	}

	async subscribeProgress(listener: (progress: JobProgress) => void): Promise<() => void> {
		this.progressListeners.add(listener);
		return () => {
			this.progressListeners.delete(listener);
		};
	}

	async getClipboard(): Promise<Clipboard> {
		return structuredClone(this.clipboard);
	}

	async setClipboard(
		mode: ClipboardMode,
		items: Location[],
		source: ClipboardSource = 'app',
	): Promise<Clipboard> {
		this.calls.push(['setClipboard', mode, items, source]);
		this.clipboard = { mode, items, source, revision: this.clipboard.revision + 1 };
		this.clipboardListeners.forEach((listener) => listener(structuredClone(this.clipboard)));
		return structuredClone(this.clipboard);
	}

	async setClipboardFromSelection(
		handle: ListingHandle,
		spec: SelectionSpec,
		mode: ClipboardMode,
	): Promise<Clipboard> {
		this.calls.push(['setClipboardFromSelection', handle, spec, mode]);
		const items = (await this.options.resolveSelection?.(handle, spec)) ?? [];
		if (items.length === 0) {
			throw refusal('copying an empty selection', 'ops', {
				kind: 'unsupported',
				what: 'copying an empty selection',
			});
		}
		return this.setClipboard(mode, items, 'app');
	}

	async resolveSelection(handle: ListingHandle, spec: SelectionSpec): Promise<Location[]> {
		this.calls.push(['resolveSelection', handle, spec]);
		const items = (await this.options.resolveSelection?.(handle, spec)) ?? [];
		if (items.length === 0) {
			throw refusal('dragging an empty selection', 'ops', {
				kind: 'unsupported',
				what: 'dragging an empty selection',
			});
		}
		return items;
	}

	async getSettings(): Promise<OpsSettings> {
		return { ...this.settings };
	}

	async setSettings(settings: OpsSettings): Promise<OpsSettings> {
		this.settings = { ...settings };
		return { ...this.settings };
	}

	async takeRecoveryReport(): Promise<RecoveryReport | null> {
		const report = this.recovery;
		this.recovery = null;
		return report;
	}

	onEvent(listener: (event: OpsEvent) => void) {
		this.eventListeners.add(listener);
		return () => {
			this.eventListeners.delete(listener);
		};
	}

	onClipboard(listener: (clipboard: Clipboard) => void) {
		this.clipboardListeners.add(listener);
		return () => {
			this.clipboardListeners.delete(listener);
		};
	}

	onRecovered(listener: (report: RecoveryReport) => void) {
		this.recoveredListeners.add(listener);
		return () => {
			this.recoveredListeners.delete(listener);
		};
	}

	// --- The script: what the worker pool and the user's answers do ---------------------------

	/** Planning finished: the job is queued (and starts, when `autoStart` is on and a slot is free). */
	planned(job: JobId): void {
		const entry = this.find(job);
		if (entry.snapshot.state.state === 'cancelled') return;
		const totals = this.totalsFor(entry.request);
		entry.snapshot.progress = {
			...entry.snapshot.progress,
			itemsTotal: totals.items,
			bytesTotal: totals.bytes,
		};
		entry.snapshot.sources = this.summarise(entry.request);
		this.go(entry, { state: 'queued' }, 'finish planning');
		this.startQueued();
	}

	/** Starts a queued job on a free slot. */
	start(job: JobId): void {
		const entry = this.find(job);
		if (entry.snapshot.state.state === 'queued' && this.slotsInUse() >= this.options.concurrency) {
			throw refusal('every worker slot is taken');
		}
		entry.snapshot.startedMs ??= this.options.now();
		this.go(entry, { state: 'running' }, 'start');
	}

	/**
	 * A progress report that passed the gate. It goes to the subscribed windows on the progress
	 * channel and uses up a revision (so events have gaps); the job's state does not change, so no
	 * event is made.
	 */
	tick(job: JobId, progress: Partial<Progress>, counts?: Partial<Counts>): void {
		const entry = this.find(job);
		const state = entry.snapshot.state.state;
		if (state !== 'running' && state !== 'planning') return;
		entry.snapshot.progress = { ...entry.snapshot.progress, ...progress };
		if (counts) entry.snapshot.counts = { ...entry.snapshot.counts, ...counts };
		this.revision += 1;
		const tick: JobProgress = {
			job,
			revision: this.revision,
			progress: structuredClone(entry.snapshot.progress),
			counts: { ...entry.snapshot.counts },
		};
		this.progressListeners.forEach((listener) => listener(tick));
	}

	/** The job finished. With `undoable` the journal records an entry of that label, as it does for most jobs. */
	done(job: JobId, undoable?: string): void {
		const entry = this.find(job);
		const total = entry.snapshot.progress;
		entry.snapshot.progress = {
			...total,
			itemsDone: total.itemsTotal,
			bytesDone: total.bytesTotal,
			etaMs: null,
			speedBps: 0,
		};
		entry.snapshot.finishedMs = this.options.now();
		this.go(entry, { state: 'done' }, 'finish');
		const kind = entry.snapshot.kind;
		if (kind.kind === 'undo' || kind.kind === 'redo') {
			const row = this.journal.find((r) => r.id === kind.of);
			if (row) {
				row.applied = kind.kind === 'redo';
				row.undoneAt = row.applied ? 0 : this.options.now() + this.journalRevision;
				this.journalChanged();
			}
		} else if (undoable !== undefined) {
			const row: JournalRow = {
				id: this.nextJournal++,
				label: undoable,
				atMs: this.options.now(),
				applied: true,
				undoneAt: 0,
			};
			this.journal.push(row);
			this.entryOfJob.set(job, row.id);
			entry.snapshot.undoable = true;
			this.changed(entry);
			this.journalChanged();
		}
		this.startQueued();
	}

	/** The job ended on an error after completing `done` items. */
	fail(job: JobId, error: OpsError, item?: Location, done = 0): void {
		const entry = this.find(job);
		entry.snapshot.finishedMs = this.options.now();
		this.go(entry, { state: 'failed', error, item: item ?? null, done }, 'fail');
		this.startQueued();
	}

	/** The job stops to ask about names that are taken. */
	askConflicts(job: JobId, conflicts: Conflict[]): void {
		this.go(this.find(job), { state: 'waiting', reason: { kind: 'conflicts', conflicts } }, 'wait');
	}

	/** The job stops to ask what to do about an item that failed. */
	askError(job: JobId, error: OpsError, item: Location): void {
		this.go(this.find(job), { state: 'waiting', reason: { kind: 'error', error, item } }, 'wait');
	}

	/** A cancelling job finished unwinding. */
	settleCancel(job: JobId): void {
		const entry = this.find(job);
		entry.snapshot.finishedMs = this.options.now();
		this.go(entry, { state: 'cancelled' }, 'finish cancelling');
		this.startQueued();
	}

	/** Leaves a report for `takeRecoveryReport` to hand over once. */
	setRecoveryReport(report: RecoveryReport | null): void {
		this.recovery = report;
	}

	/** Sends the report on the broadcast event, as the plugin does at start-up. */
	emitRecovered(report: RecoveryReport): void {
		this.recoveredListeners.forEach((listener) => listener(report));
	}

	/**
	 * Makes the next `count` events go astray: the state changes and the revision counts, but no
	 * listener hears them, as when a window missed some. The next event it does hear shows the gap.
	 */
	dropNextEvents(count: number): void {
		this.mute = count;
	}

	/** The jobs as the queue holds them now, for assertions. */
	jobs(): JobSnapshot[] {
		return this.entries.map((entry) => entry.snapshot);
	}

	// --- Internals -----------------------------------------------------------------------------

	private requestFor(kind: JobKind, destination: Location | null): JobRequest {
		return {
			kind,
			sources: { kind: 'locations', locations: [] },
			destination,
			name: null,
			options: { conflict: null, verify: null },
			originWindow: 'fake',
		};
	}

	private totalsFor(request: JobRequest): { items: number; bytes: number } {
		if (this.options.totals) return this.options.totals(request);
		const count = request.sources.kind === 'locations' ? request.sources.locations.length : 1;
		return { items: Math.max(count, 1), bytes: 0 };
	}

	private summarise(request: JobRequest): JobSnapshot['sources'] {
		// A create has no source; the planner reports the name it made, which the fake takes as asked.
		if (request.kind.kind === 'createFolder' || request.kind.kind === 'createFile') {
			return { count: 0, first: request.name };
		}
		if (request.sources.kind === 'locations') {
			const first = request.sources.locations[0];
			return {
				count: request.sources.locations.length,
				first: first ? basename(first.display) : null,
			};
		}
		return { count: null, first: null };
	}

	private add(request: JobRequest, title?: string): JobId {
		const id = this.nextJob++ as JobId;
		const kind = request.kind;
		const snapshot: JobSnapshot = {
			id,
			kind,
			state: { state: 'planning' },
			title:
				title ??
				`${kind.kind} ${request.sources.kind === 'locations' ? request.sources.locations.length : 0}`,
			sources: this.summarise(request),
			destination: request.destination,
			options: request.options,
			originWindow: request.originWindow,
			counts: { skipped: 0, failed: 0 },
			progress: {
				itemsDone: 0,
				itemsTotal: 0,
				bytesDone: 0,
				bytesTotal: 0,
				current: null,
				speedBps: 0,
				etaMs: null,
			},
			createdMs: this.options.now(),
			startedMs: null,
			finishedMs: null,
			undoable: false,
			verified: null,
		};
		this.entries.push({ snapshot, request, resolved: [] });
		this.revision += 1;
		this.emit({ kind: 'jobAdded', job: structuredClone(snapshot), revision: this.revision });
		return id;
	}

	private remove(entry: Entry): void {
		if (!FINISHED.has(entry.snapshot.state.state)) this.illegal(entry, 'dismiss');
		this.entries.splice(this.entries.indexOf(entry), 1);
		this.revision += 1;
		this.emit({ kind: 'jobRemoved', id: entry.snapshot.id, revision: this.revision });
	}

	private find(job: JobId): Entry {
		const entry = this.entries.find((e) => e.snapshot.id === job);
		if (!entry) throw refusal(`there is no job ${job}`);
		return entry;
	}

	private illegal(entry: Entry, action: string): never {
		throw refusal(`job ${entry.snapshot.id} is ${entry.snapshot.state.state} and cannot ${action}`);
	}

	private go(entry: Entry, next: JobState, action: string): void {
		if (!isLegalTransition(entry.snapshot.state.state, next.state)) this.illegal(entry, action);
		entry.snapshot.state = next;
		this.changed(entry);
	}

	private changed(entry: Entry): void {
		this.revision += 1;
		this.emit({
			kind: 'jobChanged',
			job: structuredClone(entry.snapshot),
			revision: this.revision,
		});
	}

	private journalChanged(): void {
		this.journalRevision += 1;
		const { journal } = this.currentSnapshot();
		this.emit({
			kind: 'journalChanged',
			revision: this.journalRevision,
			undo: journal.undo,
			redo: journal.redo,
		});
	}

	private slotsInUse(): number {
		return this.entries.filter((e) =>
			['running', 'paused', 'waiting', 'cancelling'].includes(e.snapshot.state.state),
		).length;
	}

	private startQueued(): void {
		if (!this.options.autoStart) return;
		for (const entry of this.entries) {
			if (this.slotsInUse() >= this.options.concurrency) return;
			if (entry.snapshot.state.state === 'queued') this.start(entry.snapshot.id);
		}
	}

	private emit(event: OpsEvent): void {
		if (this.mute > 0) {
			this.mute -= 1;
			return;
		}
		this.eventListeners.forEach((listener) => listener(event));
	}
}

export function createFakeOpsClient(options?: FakeOpsOptions): FakeOpsClient {
	return new FakeOpsClient(options);
}
