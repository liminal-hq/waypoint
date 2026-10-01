// Speaks the queue's milestones through a live region: starts, endings, waits and a few steps of long jobs, never every tick
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import { tf, tn } from '../i18n/messages';
import { errorText, jobDoneText, jobTitle } from './jobText';
import { isFinished, jobFraction, withProgress } from './opsSelectors';
import type { OpsHandle } from './opsStore';

/** A job must have been running this long before its steps are spoken; a quick one only starts and ends. */
export const MILESTONE_AFTER_MS = 3000;
/** Steps of a long job, as percentages. 100 is the end, which is announced as an ending. */
export const MILESTONES = [25, 50, 75] as const;

export interface OpsAnnouncerOptions {
	/** Says `text` politely (the app's live region). */
	announce: (text: string) => void;
	/** Which jobs to speak for; every job by default. A window speaks for the jobs it started. */
	include?: (job: JobSnapshot) => boolean;
	/** The clock, in milliseconds. */
	now?: () => number;
}

interface Seen {
	state: JobSnapshot['state']['state'];
	/** The clock when the job first ran, or `null` before it has. */
	startedAt: number | null;
	/** The highest milestone already passed (spoken or too early to speak). */
	milestone: number;
}

/**
 * Follows the store and announces:
 * - a job starting to run (once), with how many others are still in progress;
 * - a job waiting for the person, failing, being cancelled or finishing;
 * - 25, 50 and 75 % of a job that has run for at least `MILESTONE_AFTER_MS`, at most one message
 *   per progress update, with the items done.
 *
 * Jobs already in the queue when it starts are not announced. Returns what stops it.
 */
export function startOpsAnnouncer(handle: OpsHandle, options: OpsAnnouncerOptions): () => void {
	const include = options.include ?? (() => true);
	const now = options.now ?? (() => Date.now());
	const seen = new Map<number, Seen>();
	let seeded = false;

	const process = () => {
		const { snapshot, jobRevisions } = handle.store.getState();
		if (!snapshot) return;
		const { ticks } = handle.progress.getState();
		const unfinished = snapshot.jobs.filter((job) => !isFinished(job)).length;
		const others = (job: JobSnapshot) => unfinished - (isFinished(job) ? 0 : 1);
		const remaining = (job: JobSnapshot) => {
			const n = others(job);
			return n > 0 ? ` ${tn('ops.announce.remaining', n)}` : '';
		};
		const ids = new Set(snapshot.jobs.map((job) => job.id));
		for (const id of seen.keys()) if (!ids.has(id)) seen.delete(id);

		for (const job of snapshot.jobs) {
			const state = job.state.state;
			const before = seen.get(job.id);
			const entry: Seen = before ?? { state, startedAt: null, milestone: 0 };
			const hadStarted = before?.startedAt != null;
			if (state === 'running' && entry.startedAt === null) entry.startedAt = now();
			seen.set(job.id, entry);
			const speak = seeded && include(job);

			if (speak && (!before || before.state !== state)) {
				const first = state === 'running' && !hadStarted;
				if (first) {
					options.announce(tf('ops.announce.started', { title: jobTitle(job) }) + remaining(job));
				} else if (state === 'waiting') {
					options.announce(tf('ops.announce.waiting', { title: jobTitle(job) }));
				} else if (job.state.state === 'failed') {
					options.announce(
						tf('ops.announce.failed', {
							title: jobTitle(job),
							reason: errorText(job.state.error),
						}) + remaining(job),
					);
				} else if (state === 'cancelled') {
					options.announce(tf('ops.announce.cancelled', { title: jobTitle(job) }) + remaining(job));
				} else if (state === 'done') {
					options.announce(`${jobDoneText(job)}.${remaining(job)}`);
				}
			}
			entry.state = state;

			if (state === 'running') {
				const { progress } = withProgress(job, jobRevisions[job.id] ?? 0, ticks[job.id]);
				const fraction = jobFraction(progress);
				if (fraction !== null) {
					const percent = fraction * 100;
					const reached = MILESTONES.filter((m) => m <= percent && m > entry.milestone);
					const top = reached[reached.length - 1];
					if (top !== undefined) {
						entry.milestone = top;
						const long = entry.startedAt !== null && now() - entry.startedAt >= MILESTONE_AFTER_MS;
						if (speak && long) {
							options.announce(
								tf('ops.announce.milestone', {
									title: jobTitle(job),
									percent: top,
									done: progress.itemsDone,
									total: progress.itemsTotal,
								}),
							);
						}
					}
				}
			}
		}
		seeded = true;
	};

	const stopState = handle.store.subscribe(process);
	const stopProgress = handle.progress.subscribe(process);
	process();
	return () => {
		stopState();
		stopProgress();
	};
}
