// The words for a job: its title, route, state, progress and errors, and the actions its state allows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import { formatSize } from '../browse/format';
import { t, tf, tn, type MessageId, type PluralId } from '../i18n/messages';
import type { Location } from '../services/opsClient';
import { isFinished, jobFraction, type JobWithProgress } from './opsSelectors';

/** The kinds this area words itself; the others show the title Rust made. */
const WORDED = new Set(['copy', 'move', 'link', 'trash', 'delete', 'duplicate', 'restore']);

/** Kinds that act on one entry and are worded only by its name (a create has no source, so `first` is what it made). */
const NAMED_ONLY = new Set(['createFolder', 'createFile', 'rename']);

export type JobAction =
	'pause' | 'resume' | 'cancel' | 'retry' | 'dismiss' | 'resolve' | 'showInFolder';

export interface JobView {
	id: number;
	state: JobSnapshot['state']['state'];
	title: string;
	/** "source → destination", or what the job works on when it has no destination. */
	route: string;
	stateText: string;
	/** 0 to 1, or `null` before the job is sized. */
	fraction: number | null;
	/** Whether the row draws a progress bar. */
	showProgress: boolean;
	/** Speed and time left, and how far in items or bytes. */
	detail: string;
	actions: JobAction[];
	/** The job is queued and can move up or down the queue. */
	canMoveUp: boolean;
	canMoveDown: boolean;
	/** This job's place among the queued jobs, from 0, or `null` when it is not queued. */
	queuePosition: number | null;
	attention: 'waiting' | 'failed' | null;
	destination: Location | null;
	undoable: boolean;
	finished: boolean;
}

export function baseName(display: string): string {
	const parts = display.split(/[\\/]/).filter(Boolean);
	return parts[parts.length - 1] ?? display;
}

function sourcesText(job: JobSnapshot): string {
	const { count, first } = job.sources;
	if (count === 1 && first) return first;
	if (count !== null) return tn('ops.sources', count);
	return '';
}

function wordedTitle(prefix: 'ops.title' | 'ops.done', job: JobSnapshot): string | null {
	const kind = job.kind.kind;
	const { count, first } = job.sources;
	if (NAMED_ONLY.has(kind)) {
		return first ? tf(`${prefix}.${kind}.named` as MessageId, { name: first }) : null;
	}
	if (!WORDED.has(kind)) return null;
	if (count === 1 && first) return tf(`${prefix}.${kind}.named` as MessageId, { name: first });
	if (count !== null) return tn(`${prefix}.${kind}` as PluralId, count);
	return null;
}

/** What the job is doing, in the present tense ("Copying 3 items"). */
export function jobTitle(job: JobSnapshot): string {
	return wordedTitle('ops.title', job) ?? job.title;
}

/** What the job did, in the past tense ("Copied 3 items"), for the toast when it ends. */
export function jobDoneText(job: JobSnapshot): string {
	return wordedTitle('ops.done', job) ?? tf('ops.done.generic', { title: jobTitle(job) });
}

export function errorText(error: OpsError): string {
	const key = `ops.error.${error.kind}` as MessageId;
	switch (error.kind) {
		case 'notFound':
		case 'permissionDenied':
		case 'nameInUse':
		case 'protected':
		case 'changedSince':
		case 'verifyFailed':
		case 'cannotReplace':
			return tf(key, { name: baseName(error.location.display) });
		case 'undoStale':
			return tf(`ops.error.undoStale.${error.reason}` as MessageId, {
				name: baseName(error.location.display),
			});
		case 'invalidName':
			return tf(key, { name: error.name });
		default:
			return t(key);
	}
}

function formatDuration(ms: number): string {
	const seconds = Math.max(1, Math.round(ms / 1000));
	if (seconds < 60) return tf('ops.duration.seconds', { n: seconds });
	const minutes = Math.round(seconds / 60);
	if (minutes < 60) return tf('ops.duration.minutes', { n: minutes });
	return tf('ops.duration.hours', { h: Math.floor(minutes / 60), m: minutes % 60 });
}

function detailText({ job, progress }: JobWithProgress): string {
	const parts: string[] = [];
	if (progress.bytesTotal > 0) {
		parts.push(
			tf('ops.progress.bytes', {
				done: formatSize(progress.bytesDone),
				total: formatSize(progress.bytesTotal),
			}),
		);
	} else if (progress.itemsTotal > 0) {
		parts.push(tf('ops.progress.items', { done: progress.itemsDone, total: progress.itemsTotal }));
	}
	if (job.state.state === 'running') {
		if (progress.speedBps > 0) {
			parts.push(tf('ops.progress.speed', { speed: formatSize(progress.speedBps) }));
		}
		if (progress.etaMs !== null) {
			parts.push(tf('ops.progress.eta', { time: formatDuration(progress.etaMs) }));
		}
	}
	return parts.join(' · ');
}

export function stateText(job: JobSnapshot): string {
	const state = job.state;
	switch (state.state) {
		case 'waiting':
			return state.reason.kind === 'conflicts'
				? tn('ops.state.waiting.conflicts', state.reason.conflicts.length)
				: tf('ops.state.waiting.error', { reason: errorText(state.reason.error) });
		case 'failed':
			return tf('ops.state.failed', { reason: errorText(state.error) });
		case 'done': {
			const skipped = job.counts.skipped;
			return skipped > 0
				? `${t('ops.state.done')} · ${tn('ops.skipped', skipped)}`
				: t('ops.state.done');
		}
		default:
			return t(`ops.state.${state.state}` as MessageId);
	}
}

function actionsFor(job: JobSnapshot, canShow: boolean): JobAction[] {
	switch (job.state.state) {
		case 'planning':
		case 'queued':
			return ['cancel'];
		case 'running':
			return ['pause', 'cancel'];
		case 'paused':
			return ['resume', 'cancel'];
		case 'waiting':
			return ['resolve', 'cancel'];
		case 'cancelling':
			return [];
		case 'done':
			return [...(canShow && job.destination ? (['showInFolder'] as const) : []), 'dismiss'];
		case 'failed':
		case 'cancelled':
			return ['retry', 'dismiss'];
	}
}

/**
 * The rows for the queue UI, in queue order. `canShow` says whether this window can show a
 * location (the main windows can, the Operations window cannot).
 */
export function jobViews(items: readonly JobWithProgress[], canShow: boolean): JobView[] {
	const queued = items.filter((item) => item.job.state.state === 'queued');
	return items.map((item) => {
		const { job, progress } = item;
		const state = job.state.state;
		const from = sourcesText(job);
		const to = job.destination ? baseName(job.destination.display) : '';
		const queuePosition = state === 'queued' ? queued.indexOf(item) : null;
		return {
			id: job.id,
			state,
			title: jobTitle(job),
			route: from && to ? tf('ops.route', { from, to }) : from || to,
			stateText: stateText(job),
			fraction: jobFraction(progress),
			showProgress: state === 'running' || state === 'paused' || state === 'waiting',
			detail: isFinished(job) || state === 'queued' ? '' : detailText(item),
			actions: actionsFor(job, canShow),
			canMoveUp: queuePosition !== null && queuePosition > 0,
			canMoveDown: queuePosition !== null && queuePosition < queued.length - 1,
			queuePosition,
			attention: state === 'waiting' ? 'waiting' : state === 'failed' ? 'failed' : null,
			destination: job.destination,
			undoable: job.undoable,
			finished: isFinished(job),
		};
	});
}
