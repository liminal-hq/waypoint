// The operations queue as a client-side copy of Rust's: one snapshot kept current by revision-gated events, and progress apart
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type {
	JobId,
	JobProgress,
	JobRequest,
	JournalId,
	Location,
	OpsClient,
	OpsEvent,
	OpsSnapshot,
} from '../services/opsClient';
import { applyOpsEvent } from './opsApply';
import { isFinished } from './opsSelectors';

export interface OpsState {
	/** `null` until the first snapshot arrives. */
	snapshot: OpsSnapshot | null;
	/** The queue revision of the event or snapshot that last set each job, to tell a progress tick is newer. */
	jobRevisions: Record<number, number>;
	/** How many times the mirror read a fresh snapshot after the first, because it had missed events. */
	resyncs: number;
}

/** Progress is not revisioned state: the latest tick per job, flushed once per animation frame. */
export interface ProgressState {
	ticks: Record<number, JobProgress>;
}

/** Calls `flush` on the next frame and returns what cancels it. */
export type FrameScheduler = (flush: () => void) => () => void;

const animationFrame: FrameScheduler = (flush) => {
	if (typeof requestAnimationFrame === 'function') {
		const id = requestAnimationFrame(flush);
		return () => cancelAnimationFrame(id);
	}
	const id = setTimeout(flush, 16);
	return () => clearTimeout(id);
};

export interface OpsHandle {
	client: OpsClient;
	store: StoreApi<OpsState>;
	progress: StoreApi<ProgressState>;
	/** Settles once the first snapshot is in and progress is subscribed. */
	ready: Promise<void>;
	/** Reads a fresh snapshot and replays the events that arrived meanwhile. */
	resync(): Promise<void>;
	/** Applies the progress ticks waiting for the next frame now. */
	flushProgress(): void;
	/** Puts a request on the queue: what the file commands call. Resolves to the job's id. */
	submitJob(request: JobRequest): Promise<JobId>;
	/** Undoes an entry of the history (the newest when omitted). */
	undo(entry?: JournalId): Promise<JobId>;
	redo(entry?: JournalId): Promise<JobId>;
	/** The unfinished jobs that touch `location`, which the plugin works out from the requests. */
	jobsTargeting(location: Location): Promise<JobId[]>;
	/** Stops following the queue. The stores keep what they hold. */
	dispose(): void;
}

export interface OpsStoreOptions {
	/** Defaults to `requestAnimationFrame`. */
	scheduleFrame?: FrameScheduler;
}

/**
 * Subscribes to events and progress first and reads the snapshot second, so no change falls between
 * the two: events that arrive before the snapshot are held and applied on top of it (those already
 * in it are recognised by their revision and skipped). An event that shows a miss (see
 * `applyOpsEvent`) starts a resync the same way. Rust owns the queue; nothing here changes it except
 * by applying what Rust says happened.
 */
export function createOpsStore(client: OpsClient, options: OpsStoreOptions = {}): OpsHandle {
	const store = createStore<OpsState>()(() => ({ snapshot: null, jobRevisions: {}, resyncs: 0 }));
	const progress = createStore<ProgressState>()(() => ({ ticks: {} }));
	const scheduleFrame = options.scheduleFrame ?? animationFrame;

	let disposed = false;
	let reading = false;
	let held: OpsEvent[] = [];
	let pending = new Map<number, JobProgress>();
	let cancelFrame: (() => void) | null = null;
	let scheduled = false;

	function prune(snapshot: OpsSnapshot): void {
		const live = new Set(snapshot.jobs.filter((job) => !isFinished(job)).map((job) => job.id));
		const { ticks } = progress.getState();
		const kept = Object.keys(ticks).filter((id) => live.has(Number(id)));
		if (kept.length !== Object.keys(ticks).length) {
			progress.setState({ ticks: Object.fromEntries(kept.map((id) => [id, ticks[Number(id)]!])) });
		}
		for (const id of [...pending.keys()]) if (!live.has(id)) pending.delete(id);
	}

	function flushProgress(): void {
		cancelFrame?.();
		cancelFrame = null;
		scheduled = false;
		if (pending.size === 0) return;
		const { snapshot } = store.getState();
		const next = { ...progress.getState().ticks };
		for (const [id, tick] of pending) {
			const job = snapshot?.jobs.find((j) => j.id === id);
			if (job && !isFinished(job)) next[id] = tick;
		}
		pending = new Map();
		progress.setState({ ticks: next });
	}

	function applyEvent(event: OpsEvent): boolean {
		const { snapshot, jobRevisions } = store.getState();
		if (!snapshot) return false;
		const result = applyOpsEvent(snapshot, event);
		if (!result.applied) return false;
		const revisions = { ...jobRevisions };
		if (event.kind === 'jobAdded' || event.kind === 'jobChanged') {
			revisions[event.job.id] = event.revision;
		} else if (event.kind === 'jobRemoved') {
			delete revisions[event.id];
		}
		store.setState({ snapshot: result.snapshot, jobRevisions: revisions });
		prune(result.snapshot);
		return result.resync;
	}

	async function read(first: boolean): Promise<void> {
		if (reading) return;
		reading = true;
		try {
			const fresh = await client.snapshot();
			if (disposed) return;
			const revisions: Record<number, number> = {};
			for (const job of fresh.jobs) revisions[job.id] = fresh.revision;
			store.setState((state) => ({
				snapshot: fresh,
				jobRevisions: revisions,
				resyncs: first ? state.resyncs : state.resyncs + 1,
			}));
			prune(fresh);
			const replay = held;
			held = [];
			let again = false;
			for (const event of replay) again = applyEvent(event) || again;
			reading = false;
			// Replayed events that showed another miss: the snapshot was older than they are.
			if (again) await read(false);
		} finally {
			reading = false;
		}
	}

	const unsubscribeEvents = client.onEvent((event) => {
		if (disposed) return;
		if (reading || store.getState().snapshot === null) {
			held.push(event);
			return;
		}
		if (applyEvent(event)) void read(false);
	});

	let unsubscribeProgress: (() => void) | null = null;
	const subscribed = client
		.subscribeProgress((tick) => {
			if (disposed) return;
			const known = pending.get(tick.job) ?? progress.getState().ticks[tick.job];
			if (known && known.revision >= tick.revision) return;
			pending.set(tick.job, tick);
			if (!scheduled) {
				scheduled = true;
				cancelFrame = scheduleFrame(flushProgress);
			}
		})
		.then(
			(stop) => {
				if (disposed) void stop();
				else unsubscribeProgress = stop;
			},
			(error: unknown) => console.warn('could not subscribe to operation progress', error),
		);

	const ready = Promise.all([subscribed, read(true)]).then(() => undefined);

	return {
		client,
		store,
		progress,
		ready,
		resync: () => read(false),
		flushProgress,
		submitJob: (request) => client.submit(request),
		undo: (entry) => client.undo(entry),
		redo: (entry) => client.redo(entry),
		jobsTargeting: (location) => client.jobsTargeting(location),
		dispose() {
			disposed = true;
			cancelFrame?.();
			cancelFrame = null;
			unsubscribeEvents();
			unsubscribeProgress?.();
			unsubscribeProgress = null;
		},
	};
}
