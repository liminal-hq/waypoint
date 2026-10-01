// The ring's state worked out from the queue: which urgency it shows, its fill, count and spoken label
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tf, tn } from '../i18n/messages';
import {
	mostUrgent,
	overallProgress,
	unfinishedCount,
	type JobWithProgress,
	type Urgency,
} from './opsSelectors';

export interface RingModel {
	urgency: Urgency;
	/** Jobs that are not finished: the number shown beside the ring. */
	count: number;
	/** 0 to 1, or `null` when nothing is sized (a quiet ring, or one for jobs not yet planned). */
	fraction: number | null;
	label: string;
}

export function ringModel(jobs: readonly JobWithProgress[]): RingModel {
	const plain = jobs.map((item) => item.job);
	const urgency = mostUrgent(plain);
	const count = unfinishedCount(plain);
	const { fraction } = overallProgress(jobs);
	const parts: string[] = [];
	if (urgency === 'idle') parts.push(t('ops.ring.idle'));
	else if (urgency === 'done') parts.push(t('ops.ring.done'));
	else parts.push(count > 0 ? tn('ops.ring.active', count) : t('ops.ring.idle'));
	if (count > 0 && fraction !== null) {
		parts.push(tf('ops.ring.percent', { percent: Math.round(fraction * 100) }));
	}
	if (urgency === 'waiting') parts.push(t('ops.ring.waiting'));
	if (jobs.some((item) => item.job.state.state === 'failed') && urgency !== 'failed') {
		// A failure behind a wait is still worth saying.
		parts.push(t('ops.ring.failed'));
	}
	if (urgency === 'failed') parts.push(t('ops.ring.failed'));
	return { urgency, count, fraction: count > 0 ? fraction : null, label: parts.join(', ') };
}
