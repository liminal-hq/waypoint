// The words for a job: its title, route, state, progress and errors, and the actions its state allows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import type { LeftOutName } from '@liminal-hq/waypoint-protocol/generated/LeftOutName';
import type { LeftOutNote } from '@liminal-hq/waypoint-protocol/generated/LeftOutNote';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import { formatCount, formatSize } from '../browse/format';
import { connectionErrorText } from '../connections/connectModel';
import { formatLocale } from '../i18n/active';
import { t, tf, tn, type MessageId, type PluralId } from '../i18n/messages';
import type { JobPriority, Location, Schedule } from '../services/opsClient';
import { archiveRefusalText } from './archiveRefusal';
import { isFinished, jobFraction, type JobWithProgress } from './opsSelectors';

/** The kinds this area words itself; the others show the title Rust made. */
const WORDED = new Set([
	'copy',
	'move',
	'link',
	'trash',
	'delete',
	'duplicate',
	'restore',
	'extract',
	'compress',
]);

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
	/** The entries an extraction left out, one line each, for a disclosure under the state; empty for every other job. */
	leftOut: string[];
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
	/** This job's own speed limit in bytes a second, or `null` for none. */
	speedLimit: number | null;
	priority: JobPriority;
	/** Whether the row offers a speed limit: a copy or move that has not finished. */
	canLimit: boolean;
	/** Whether the row offers a priority: a job that still waits for a slot. */
	canPrioritise: boolean;
	/** When the job may start, or `null` for as soon as a slot is free. */
	schedule: Schedule | null;
	/** Whether the row offers a schedule: a job that has not started. */
	canSchedule: boolean;
	/** The server the job uploads to, downloads from or copies between, in words; `''` for a local job. */
	server: string;
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

/** The entries shown by name in a notice; the rest are counted. */
const LEFT_OUT_NAMED = 3;

/** Why an entry was left out, in words. */
export function leftOutWhyText(why: LeftOutName['why']): string {
	return t(`ops.leftOut.why.${why}` as MessageId);
}

/** The sentence that says which entries an extraction left out, or `''` when it left none out. */
export function leftOutSentence(note: LeftOutNote | undefined): string {
	if (!note || note.count === 0) return '';
	const named = note.shown.slice(0, LEFT_OUT_NAMED).map((entry) => entry.name);
	const rest = note.count - named.length;
	const parts = rest > 0 ? [...named, tf('ops.leftOut.more', { count: formatCount(rest) })] : named;
	return tn('ops.leftOut.sentence', note.count, undefined, { names: listOf(parts) });
}

/** One line for each entry in a note's details, as the Operations list shows them. */
export function leftOutLines(note: LeftOutNote | undefined): string[] {
	if (!note) return [];
	const lines = note.shown.map((entry) =>
		tf('ops.leftOut.line', { name: entry.name, why: leftOutWhyText(entry.why) }),
	);
	const rest = note.count - note.shown.length;
	return rest > 0 ? [...lines, tf('files.delete.more', { count: formatCount(rest) })] : lines;
}

function listOf(parts: string[]): string {
	return new Intl.ListFormat(formatLocale(), { style: 'long', type: 'conjunction' }).format(parts);
}

/** What the job did, in the past tense ("Copied 3 items"), for the toast when it ends. */
export function jobDoneText(job: JobSnapshot): string {
	const done = wordedTitle('ops.done', job) ?? tf('ops.done.generic', { title: jobTitle(job) });
	const left = leftOutSentence(job.leftOut);
	return left ? `${done}. ${left}` : done;
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
		case 'archiveLimit':
			return tf(key, { name: baseName(error.location.display) });
		case 'archiveNotWritable':
			return archiveRefusalText(error.location, error.reason);
		case 'undoStale':
			return tf(`ops.error.undoStale.${error.reason}` as MessageId, {
				name: baseName(error.location.display),
			});
		case 'invalidName':
			return tf(key, { name: error.name });
		case 'connection':
			// The words the Connect dialog and a server's tab use, so a lost connection reads the same everywhere.
			return connectionErrorText(error.error);
		default:
			return t(key);
	}
}

/**
 * The server a job reaches, in words ("Uploading to nas.lan", "From nas.lan", "From nas.lan to
 * backup"), from the logins Rust found while planning; `name` turns a login into what the person
 * calls it. `''` for a job between local folders.
 */
export function serverText(job: JobSnapshot, name: (login: string) => string): string {
	const ends = job.ends;
	if (!ends) return '';
	const from = ends.from[0];
	if (ends.to !== null && from !== undefined && from !== ends.to) {
		return tf('ops.server.between', { from: name(from), to: name(ends.to) });
	}
	if (ends.to !== null) {
		return tf(from === undefined ? 'ops.server.to' : 'ops.server.on', { server: name(ends.to) });
	}
	return from === undefined ? '' : tf('ops.server.from', { server: name(from) });
}

/** What the copies could not keep, in words, or `''` when they kept everything. */
function droppedText(job: JobSnapshot): string {
	const dropped = job.dropped ?? [];
	if (dropped.length === 0) return '';
	const times = dropped.includes('modifiedTimes');
	const permissions = dropped.includes('permissions');
	return t(
		times && permissions
			? 'ops.dropped.both'
			: times
				? 'ops.dropped.modifiedTimes'
				: 'ops.dropped.permissions',
	);
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
		case 'offline':
			// A lost server is waited for and tried again by itself (D165): say so, and which try it is.
			return tf('ops.state.offline', { reason: errorText(state.error), attempt: state.attempt });
		case 'done': {
			const skipped = job.counts.skipped;
			const dropped = droppedText(job);
			return [
				t('ops.state.done'),
				...(skipped > 0 ? [tn('ops.skipped', skipped)] : []),
				...(job.leftOut ? [tn('ops.leftOut', job.leftOut.count)] : []),
				...(dropped ? [dropped] : []),
			].join(' · ');
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
		case 'offline':
			return ['cancel'];
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
export function jobViews(
	items: readonly JobWithProgress[],
	canShow: boolean,
	serverName: (login: string) => string = (login) => login,
): JobView[] {
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
			leftOut: leftOutLines(job.leftOut),
			fraction: jobFraction(progress),
			showProgress:
				state === 'running' || state === 'paused' || state === 'waiting' || state === 'offline',
			detail: isFinished(job) || state === 'queued' ? '' : detailText(item),
			actions: actionsFor(job, canShow),
			canMoveUp: queuePosition !== null && queuePosition > 0,
			canMoveDown: queuePosition !== null && queuePosition < queued.length - 1,
			queuePosition,
			attention: state === 'waiting' ? 'waiting' : state === 'failed' ? 'failed' : null,
			destination: job.destination,
			undoable: job.undoable,
			finished: isFinished(job),
			speedLimit: job.options.speedLimit ?? null,
			priority: job.options.priority ?? 'normal',
			canLimit: !isFinished(job) && (job.kind.kind === 'copy' || job.kind.kind === 'move'),
			canPrioritise: state === 'planning' || state === 'queued',
			schedule: job.options.schedule ?? null,
			canSchedule: state === 'planning' || state === 'queued',
			server: serverText(job, serverName),
		};
	});
}
