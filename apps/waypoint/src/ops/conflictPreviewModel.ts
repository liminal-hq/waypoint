// What the conflict dialog works out from a preview: the size comparison, how much of a diff is drawn, and what to say of each outcome
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Conflict } from '@liminal-hq/waypoint-protocol/generated/Conflict';
import type { ConflictPreview } from '@liminal-hq/waypoint-protocol/generated/ConflictPreview';
import type { DiffLine } from '@liminal-hq/waypoint-protocol/generated/DiffLine';
import type { TextDiff } from '@liminal-hq/waypoint-protocol/generated/TextDiff';

/** How many diff rows are drawn before "N more lines". */
export const DIFF_ROW_CAP = 200;

/** How many rows ask for their preview without being opened: the first ones, so a long list stays quiet. */
export const AUTO_PREVIEWS = 12;

/** How many rows ask for thumbnails. */
export const THUMBNAIL_ROWS = 40;

/** How many previews are read at once. */
export const PREVIEW_CONCURRENCY = 2;

/** Only two files that both exist can be compared. */
export function canCompare(conflict: Conflict): boolean {
	return conflict.kind === 'fileOverFile' && !conflict.withinBatch;
}

export type SizeHint = 'larger' | 'smaller' | 'same' | 'unknown';

/** How the incoming file's size compares with the existing one's. */
export function sizeHint(conflict: Conflict): SizeHint {
	const { sourceSize: incoming, existingSize: existing } = conflict;
	if (!canCompare(conflict) || incoming === null || existing === null) return 'unknown';
	if (incoming > existing) return 'larger';
	if (incoming < existing) return 'smaller';
	return 'same';
}

/** What is known of one clash's preview. */
export type PreviewState =
	| { status: 'idle' }
	| { status: 'loading' }
	| { status: 'ready'; preview: ConflictPreview }
	| { status: 'failed' };

export const IDLE: PreviewState = { status: 'idle' };

/** The rows of a diff that are drawn, and how many lines are not. Gaps are not lines. */
export function visibleDiff(
	diff: TextDiff,
	cap = DIFF_ROW_CAP,
): { lines: DiffLine[]; more: number } {
	const lines = diff.lines.slice(0, cap);
	const left = diff.lines.slice(cap).filter((line) => line.op !== 'gap').length;
	return { lines, more: diff.more + left };
}
