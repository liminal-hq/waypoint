// Which of a mark's two sides speaks for it, and where it sorts: pure and free of text, so the fake listing can use it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GitChange } from '@liminal-hq/waypoint-protocol/generated/GitChange';
import type { GitMark } from '@liminal-hq/waypoint-protocol/generated/GitMark';

/** The order Rust ranks a mark's two sides in (`GitMark::primary`): the later one wins. */
const LOUDNESS: readonly GitChange[] = [
	'modified',
	'added',
	'deleted',
	'renamed',
	'typeChanged',
	'untracked',
	'ignored',
	'conflicted',
];

/** The change the one-letter summary of a mark stands for, or `null` for a mark with none. */
export function primaryChange(mark: GitMark): GitChange | null {
	const sides = [mark.staged, mark.unstaged].filter((side): side is GitChange => side != null);
	if (sides.length === 0) return null;
	return sides.reduce((a, b) => (LOUDNESS.indexOf(b) > LOUDNESS.indexOf(a) ? b : a));
}

/** Where an entry sorts under the Git column (`GitMark::sort_rank` in Rust): conflicts, edits, new files, clean, then ignored. */
export function markSortRank(mark: GitMark | undefined): number {
	if (!mark) return 7;
	switch (primaryChange(mark)) {
		case 'conflicted':
			return 0;
		case 'modified':
			return 1;
		case 'deleted':
			return 2;
		case 'added':
			return 3;
		case 'renamed':
			return 4;
		case 'typeChanged':
			return 5;
		case 'untracked':
			return 6;
		case 'ignored':
			if ((mark.inside ?? 0) === 0) return 8;
			break;
		default:
			break;
	}
	if ((mark.conflictedInside ?? 0) > 0) return 0;
	if ((mark.inside ?? 0) > 0) return 6;
	return 7;
}
