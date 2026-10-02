// What the queue UI derives from the mirrored queue: counts, overall progress, the most urgent state and per-job progress
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Counts } from '@liminal-hq/waypoint-protocol/generated/Counts';
import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { Progress } from '@liminal-hq/waypoint-protocol/generated/Progress';
import type { JobProgress, Location } from '../services/opsClient';

/** A job with the freshest progress known for it. */
export interface JobWithProgress {
	job: JobSnapshot;
	progress: Progress;
	counts: Counts;
}

export function isFinished(job: JobSnapshot): boolean {
	const state = job.state.state;
	return state === 'done' || state === 'failed' || state === 'cancelled';
}

/**
 * The progress for `job`: the latest tick from the channel when it is newer than the last event
 * that changed the job (`jobRevision`), and what the snapshot holds otherwise.
 */
export function withProgress(
	job: JobSnapshot,
	jobRevision: number,
	tick: JobProgress | undefined,
): JobWithProgress {
	if (tick && tick.revision > jobRevision && !isFinished(job)) {
		return { job, progress: tick.progress, counts: tick.counts };
	}
	return { job, progress: job.progress, counts: job.counts };
}

/** Jobs that are running (not queued, paused, waiting or finished). */
export function runningCount(jobs: readonly JobSnapshot[]): number {
	return jobs.filter((job) => job.state.state === 'running').length;
}

/** Jobs that are not finished yet, whatever they are doing. */
export function unfinishedCount(jobs: readonly JobSnapshot[]): number {
	return jobs.filter((job) => !isFinished(job)).length;
}

export function waitingCount(jobs: readonly JobSnapshot[]): number {
	return jobs.filter((job) => job.state.state === 'waiting').length;
}

export function failedCount(jobs: readonly JobSnapshot[]): number {
	return jobs.filter((job) => job.state.state === 'failed').length;
}

/** What the ring shows when everything is quiet: `idle` has no jobs listed, `done` only finished ones. */
export type Urgency = 'idle' | 'done' | 'running' | 'failed' | 'waiting';

/** The state that most needs the person: waiting for them, then failed, then at work, then done. */
export function mostUrgent(jobs: readonly JobSnapshot[]): Urgency {
	if (jobs.length === 0) return 'idle';
	if (jobs.some((job) => job.state.state === 'waiting')) return 'waiting';
	if (jobs.some((job) => job.state.state === 'failed')) return 'failed';
	if (jobs.some((job) => !isFinished(job))) return 'running';
	return 'done';
}

export interface OverallProgress {
	itemsDone: number;
	itemsTotal: number;
	bytesDone: number;
	bytesTotal: number;
	/** 0 to 1 by bytes when any job has a size and by items otherwise; `null` when nothing is sized yet. */
	fraction: number | null;
}

/** The progress of every unfinished job taken together. */
export function overallProgress(jobs: readonly JobWithProgress[]): OverallProgress {
	const total: OverallProgress = {
		itemsDone: 0,
		itemsTotal: 0,
		bytesDone: 0,
		bytesTotal: 0,
		fraction: null,
	};
	for (const { job, progress } of jobs) {
		if (isFinished(job)) continue;
		total.itemsDone += progress.itemsDone;
		total.itemsTotal += progress.itemsTotal;
		total.bytesDone += progress.bytesDone;
		total.bytesTotal += progress.bytesTotal;
	}
	if (total.bytesTotal > 0) total.fraction = Math.min(total.bytesDone / total.bytesTotal, 1);
	else if (total.itemsTotal > 0) total.fraction = Math.min(total.itemsDone / total.itemsTotal, 1);
	return total;
}

/** One job's fraction done, from 0 to 1, by bytes when it has any and by items otherwise; `null` before it is sized. */
export function jobFraction(progress: Progress): number | null {
	if (progress.bytesTotal > 0) return Math.min(progress.bytesDone / progress.bytesTotal, 1);
	if (progress.itemsTotal > 0) return Math.min(progress.itemsDone / progress.itemsTotal, 1);
	return null;
}

/**
 * The unfinished jobs that put something in `location` (their destination), judged from what the
 * mirror holds. `OpsClient.jobsTargeting` is the authority for the close guard, since it also knows
 * the sources; this is for a view that must answer at once.
 */
export function jobsTargetingDestination(
	jobs: readonly JobSnapshot[],
	location: Location,
): JobSnapshot[] {
	return jobs.filter((job) => !isFinished(job) && job.destination?.uri === location.uri);
}
