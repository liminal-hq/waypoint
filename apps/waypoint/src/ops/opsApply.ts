// Applies one OpsEvent to a snapshot, as the Rust store's `OpsSnapshot::apply` does, and notices when events were missed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { OpsEvent, OpsSnapshot } from '../services/opsClient';

export interface ApplyResult {
	snapshot: OpsSnapshot;
	/**
	 * The event shows that this mirror missed something (a change to a job it never heard of, an order
	 * that names other jobs, a journal revision that skipped), so the caller should read a new
	 * snapshot. A gap in the queue's revisions is not one: progress ticks use revisions up.
	 */
	resync: boolean;
	/** The event moved the snapshot on (it was not stale). */
	applied: boolean;
}

/**
 * Applies `event` to `snapshot` without changing it. An event at or below the snapshot's revision is
 * stale and ignored; a journal event is judged against the journal's own revision, which counts its
 * changes one at a time.
 */
export function applyOpsEvent(snapshot: OpsSnapshot, event: OpsEvent): ApplyResult {
	if (event.kind === 'journalChanged') {
		if (event.revision <= snapshot.journal.revision) {
			return { snapshot, resync: false, applied: false };
		}
		return {
			snapshot: {
				...snapshot,
				journal: { revision: event.revision, undo: event.undo, redo: event.redo },
			},
			resync: event.revision > snapshot.journal.revision + 1,
			applied: true,
		};
	}
	if (event.revision <= snapshot.revision) return { snapshot, resync: false, applied: false };

	const next: OpsSnapshot = { ...snapshot, revision: event.revision };
	let resync = false;
	switch (event.kind) {
		case 'jobAdded':
			next.jobs = snapshot.jobs.some((job) => job.id === event.job.id)
				? snapshot.jobs.map((job) => (job.id === event.job.id ? event.job : job))
				: [...snapshot.jobs, event.job];
			break;
		case 'jobChanged':
			if (snapshot.jobs.some((job) => job.id === event.job.id)) {
				next.jobs = snapshot.jobs.map((job) => (job.id === event.job.id ? event.job : job));
			} else {
				resync = true;
			}
			break;
		case 'jobRemoved':
			next.jobs = snapshot.jobs.filter((job) => job.id !== event.id);
			break;
		case 'queueReordered': {
			const byId = new Map<number, JobSnapshot>(snapshot.jobs.map((job) => [job.id, job]));
			const ordered: JobSnapshot[] = [];
			for (const id of event.order) {
				const job = byId.get(id);
				if (job) {
					ordered.push(job);
					byId.delete(id);
				} else {
					resync = true;
				}
			}
			// Jobs the order does not name keep their relative order at the end, as in Rust.
			if (byId.size > 0) resync = true;
			next.jobs = [...ordered, ...byId.values()];
			break;
		}
	}
	return { snapshot: next, resync, applied: true };
}
