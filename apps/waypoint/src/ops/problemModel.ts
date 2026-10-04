// The words and the choices for an item that stopped a job: one message per kind of error, and the decisions that fit it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Decision } from '@liminal-hq/waypoint-protocol/generated/Decision';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import { formatSize } from '../browse/format';
import { t, tf, type MessageId } from '../i18n/messages';

export interface ProblemText {
	/** What went wrong, in plain words. */
	message: string;
	/** Technical lines kept behind a disclosure (checksums), or none. */
	details: string[];
}

/** What went wrong with an item, in words a person can act on. */
export function problemText(error: OpsError): ProblemText {
	const key = (kind: string) => `ops.problem.message.${kind}` as MessageId;
	const details: string[] = [];
	let message: string;
	switch (error.kind) {
		case 'notFound':
		case 'permissionDenied':
		case 'nameInUse':
		case 'protected':
		case 'changedSince':
		case 'cannotReplace':
			message = tf(key(error.kind), { location: error.location.display });
			break;
		case 'verifyFailed':
			message = tf(key(error.kind), { location: error.location.display });
			details.push(
				tf('ops.problem.details.expected', { digest: error.expected }),
				tf('ops.problem.details.actual', { digest: error.actual }),
			);
			break;
		case 'archiveLimit': {
			message = tf(key(error.kind), { location: error.location.display });
			const { limit } = error;
			if (limit.kind === 'entries') {
				details.push(
					tf('ops.problem.details.archiveEntries', {
						found: String(limit.found),
						max: String(limit.max),
					}),
				);
			} else if (limit.kind === 'bytes') {
				details.push(
					tf('ops.problem.details.archiveBytes', {
						found: formatSize(limit.found),
						max: formatSize(limit.max),
					}),
				);
			} else {
				details.push(
					tf('ops.problem.details.archiveRatio', {
						ratio: String(limit.ratio),
						max: String(limit.max),
					}),
				);
			}
			break;
		}
		case 'notEnoughSpace':
			// Zero means the provider could not say how much it needed or had.
			message =
				error.needed > 0
					? tf('ops.problem.message.notEnoughSpace', {
							needed: formatSize(error.needed),
							free: formatSize(error.free),
						})
					: t('ops.problem.message.notEnoughSpace.unknown');
			break;
		case 'invalidName':
			message = tf(key(error.kind), { name: error.name, reason: error.reason });
			break;
		case 'trashUnavailable':
		case 'undoUnavailable':
			message = tf(key(error.kind), { reason: error.reason });
			break;
		case 'originMissingParent':
			message = tf(key(error.kind), { folder: error.location.display });
			break;
		case 'unsupported':
			message = tf(key(error.kind), { what: error.what });
			break;
		case 'undoStale':
			message = tf(key(error.kind), {
				reason: tf(`ops.error.undoStale.${error.reason}` as MessageId, {
					name: error.location.display,
				}),
			});
			break;
		case 'io':
			message = tf(key(error.kind), { message: error.message });
			break;
		case 'connection':
			// Every connection error names the location it was met at.
			message = tf(key(error.kind), {
				location: ('location' in error.error && error.error.location?.display) || '',
			});
			break;
		case 'sameFolder':
		case 'intoItself':
		case 'cancelled':
			message = t(key(error.kind));
			break;
	}
	return { message, details };
}

/**
 * The decisions that fit `error`, in the order they are offered. Every error can be retried,
 * skipped, skipped for good and cancelled; one whose folder is gone can also make the folder.
 */
export function decisionsFor(error: OpsError): Decision[] {
	const base: Decision[] = ['retry', 'skip', 'skipAll', 'cancel'];
	return error.kind === 'originMissingParent' ? ['createParents', ...base] : base;
}
