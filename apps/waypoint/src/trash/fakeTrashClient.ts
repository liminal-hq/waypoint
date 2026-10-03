// An in-memory TrashClient for building and testing the Trash view without the plugins
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ConflictPolicy } from '@liminal-hq/waypoint-protocol/generated/ConflictPolicy';
import type { Decision } from '@liminal-hq/waypoint-protocol/generated/Decision';
import type { Resolution } from '@liminal-hq/waypoint-protocol/generated/Resolution';
import type { JobId } from '@liminal-hq/waypoint-protocol/generated/JobId';
import type { JobKind } from '@liminal-hq/waypoint-protocol/generated/JobKind';
import type { JobRequest } from '@liminal-hq/waypoint-protocol/generated/JobRequest';
import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { JobState } from '@liminal-hq/waypoint-protocol/generated/JobState';
import type { OpsEvent } from '@liminal-hq/waypoint-protocol/generated/OpsEvent';
import type { TrashInfo } from '@liminal-hq/waypoint-protocol/generated/TrashInfo';
import type { OpsClient } from '../services/opsClient';
import type { Unsubscribe } from '../services/vfsClient';
import type { TrashClient } from './trashClient';

/** A job the fake was asked to run, with the answers it was given. */
export interface FakeJob {
	id: JobId;
	request: JobRequest;
	state: JobState;
	/** The policies the resolver answered the job's conflicts with (the bulk choice, when it sent one). */
	policies: ConflictPolicy[];
	/** The per-item answers the resolver sent with them. */
	resolutions: Resolution[];
	/** The decisions the resolver answered the job's errors with. */
	decisions: Decision[];
}

/**
 * Records the jobs it is given and lets a test play the queue: `finish`, `fail` and `wait` move a
 * job through its states and send the events a real queue would.
 */
export class FakeTrashClient implements TrashClient {
	info: TrashInfo = { available: true, reason: null, count: 0, totalBytes: null };
	/** The sizes the fake adds up when asked, as `getInfo(true)` reports them. */
	bytes: number | null = null;
	/** Whether each `getInfo` call asked for the sizes, in order. */
	readonly asked: boolean[] = [];
	readonly jobs: FakeJob[] = [];
	/** The jobs the resolver cancelled. */
	readonly cancelled: JobId[] = [];
	infoCalls = 0;
	rejectSubmits: unknown = null;
	private listeners = new Set<(event: OpsEvent) => void>();
	private revision = 0;

	constructor(info?: Partial<TrashInfo>) {
		this.info = { ...this.info, ...info };
	}

	async getInfo(withBytes = false): Promise<TrashInfo> {
		this.infoCalls += 1;
		this.asked.push(withBytes);
		return withBytes && this.info.available
			? { ...this.info, totalBytes: this.bytes }
			: { ...this.info, totalBytes: null };
	}

	async submit(request: JobRequest): Promise<JobId> {
		if (this.rejectSubmits) throw this.rejectSubmits;
		const id = this.jobs.length + 1;
		// The plugin fills in the window the request came from.
		const filled: JobRequest = { ...request, originWindow: request.originWindow || 'main-1' };
		const job: FakeJob = {
			id,
			request: filled,
			state: { state: 'queued' },
			policies: [],
			resolutions: [],
			decisions: [],
		};
		this.jobs.push(job);
		this.emit({ kind: 'jobAdded', job: this.snapshot(job), revision: ++this.revision });
		return id;
	}

	onEvent(listener: (event: OpsEvent) => void): Unsubscribe {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	/**
	 * The queue as the resolver and the queue UI see it: the same jobs and events, answered into
	 * `policies`, `resolutions`, `decisions` and `cancelled`. Only what the resolver uses is
	 * simulated; any other command rejects.
	 */
	opsClient(): OpsClient {
		const unsupported = (name: string) => () =>
			Promise.reject(new Error(`FakeTrashClient does not simulate ${name}`));
		const client: Partial<OpsClient> = {
			snapshot: async () => ({
				revision: this.revision,
				jobs: this.jobs.map((job) => this.snapshot(job)),
				journal: { revision: 0, undo: null, redo: null },
			}),
			resolve: async (job, resolutions, applyToAll) => {
				const entry = this.job(job);
				entry.resolutions.push(...resolutions);
				if (applyToAll) entry.policies.push(applyToAll);
			},
			resolveError: async (job, decision) => {
				this.job(job).decisions.push(decision);
			},
			cancel: async (job) => {
				this.cancelled.push(job);
			},
			subscribeProgress: async () => () => {},
			onEvent: (listener) => this.onEvent(listener),
			// The clipboard is not what these tests are about: an empty one that never changes.
			getClipboard: async () => ({ mode: 'copy', items: [], revision: 0, source: 'app' }),
			onClipboard: () => () => {},
			onShowJob: () => () => {},
		};
		return new Proxy(client, {
			get: (target, name) =>
				name in target ? target[name as keyof OpsClient] : unsupported(String(name)),
		}) as OpsClient;
	}

	/** The job with this id. */
	job(id: JobId): FakeJob {
		const job = this.jobs.find((j) => j.id === id);
		if (!job) throw new Error(`no job ${id}`);
		return job;
	}

	/** The last job submitted. */
	get last(): FakeJob {
		const job = this.jobs[this.jobs.length - 1];
		if (!job) throw new Error('no job was submitted');
		return job;
	}

	/** Moves a job to a state and sends the event for it. */
	move(id: JobId, state: JobState, itemsDone = 0): void {
		const job = this.job(id);
		job.state = state;
		this.emit({
			kind: 'jobChanged',
			job: this.snapshot(job, itemsDone),
			revision: ++this.revision,
		});
	}

	finish(id: JobId, itemsDone = 1): void {
		this.move(id, { state: 'done' }, itemsDone);
	}

	/** Parks a job on the clashes a restore found. */
	waitForConflicts(id: JobId, names: string[], kind: 'fileOverFile' | 'folderOverFolder'): void {
		this.move(id, {
			state: 'waiting',
			reason: {
				kind: 'conflicts',
				conflicts: names.map((name) => ({
					source: { display: `Trash/${name}`, uri: `trash:/${name}` },
					existing: { display: `/home/a/${name}`, uri: `file:///home/a/${name}` },
					name,
					kind,
					withinBatch: false,
					sourceSize: 1,
					existingSize: 2,
					sourceModifiedMs: null,
					existingModifiedMs: null,
				})),
			},
		});
	}

	/** Changes the Trash's state, as another program emptying it would. */
	setInfo(info: Partial<TrashInfo>): void {
		this.info = { ...this.info, ...info };
	}

	emit(event: OpsEvent): void {
		for (const listener of [...this.listeners]) listener(event);
	}

	private snapshot(job: FakeJob, itemsDone = 0): JobSnapshot {
		return fakeJobSnapshot(
			job.id,
			job.request.kind,
			job.state,
			itemsDone,
			job.request.originWindow,
		);
	}
}

/** A job snapshot with every field filled in, for the events a test sends. */
export function fakeJobSnapshot(
	id: JobId,
	kind: JobKind,
	state: JobState,
	itemsDone = 0,
	originWindow = 'main-1',
): JobSnapshot {
	return {
		id,
		kind,
		state,
		title: '',
		sources: { count: null, first: null },
		destination: null,
		options: { conflict: null, verify: null },
		originWindow,
		counts: { skipped: 0, failed: 0 },
		progress: {
			itemsDone,
			itemsTotal: itemsDone,
			bytesDone: 0,
			bytesTotal: 0,
			current: null,
			speedBps: 0,
			etaMs: null,
		},
		createdMs: 0,
		startedMs: null,
		finishedMs: null,
		undoable: false,
		verified: null,
	};
}
