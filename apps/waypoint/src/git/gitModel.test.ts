// Verifies what Git's marks and summaries say in words and letters, and where they sort
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GitMark } from '@liminal-hq/waypoint-protocol/generated/GitMark';
import { describe, expect, it } from 'vitest';
import { cleanSummary } from '../services/fakeGitClient';
import {
	branchText,
	CHANGE_LETTERS,
	hasStatus,
	markLetters,
	markWords,
	summaryCounts,
	summaryWords,
} from './gitModel';
import { markSortRank, primaryChange } from './markRank';

describe('letters and words', () => {
	it('gives every kind of change its own letter', () => {
		const letters = Object.values(CHANGE_LETTERS);
		expect(new Set(letters).size).toBe(letters.length);
		expect(CHANGE_LETTERS.untracked).toBe('?');
		expect(CHANGE_LETTERS.ignored).toBe('!');
		expect(CHANGE_LETTERS.conflicted).toBe('U');
	});

	it('says the staged and the unstaged side of a change apart', () => {
		expect(markWords({ staged: 'modified' })).toBe('Modified, staged');
		expect(markWords({ unstaged: 'modified' })).toBe('Modified, not staged');
		expect(markWords({ staged: 'added', unstaged: 'modified' })).toBe(
			'Added, staged; Modified, not staged',
		);
		expect(markLetters({ staged: 'added', unstaged: 'modified' })).toEqual({
			staged: 'A',
			unstaged: 'M',
		});
	});

	it('does not call an untracked, ignored or conflicted path staged or not', () => {
		expect(markWords({ unstaged: 'untracked' })).toBe('Untracked');
		expect(markWords({ unstaged: 'ignored' })).toBe('Ignored');
		expect(markWords({ unstaged: 'conflicted' })).toBe('Conflict');
	});

	it('counts what changed inside a folder, with the conflicts among them, and a repository', () => {
		expect(markWords({ inside: 1 })).toBe('1 changed item inside');
		expect(markWords({ inside: 4, conflictedInside: 2 })).toBe(
			'4 changed items inside; 2 in conflict',
		);
		expect(markWords({ repository: true })).toBe('Git repository');
		expect(markWords({})).toBe('');
	});

	it('shows a change in the column but not a plain repository', () => {
		expect(hasStatus(undefined)).toBe(false);
		expect(hasStatus({ repository: true })).toBe(false);
		expect(hasStatus({ unstaged: 'modified' })).toBe(true);
		expect(hasStatus({ inside: 2 })).toBe(true);
	});
});

describe('the branch', () => {
	it('names the branch with the distance from its upstream', () => {
		expect(branchText(cleanSummary())).toBe('main');
		expect(branchText(cleanSummary({ ahead: 2, behind: 0 }))).toBe('main ↑2');
		expect(branchText(cleanSummary({ ahead: 2, behind: 5 }))).toBe('main ↑2 ↓5');
		expect(branchText(cleanSummary({ behind: 10000, distanceCapped: true }))).toBe('main ↓10000+');
	});

	it('says a detached head and a branch with no commits in words', () => {
		expect(branchText(cleanSummary({ headKind: 'detached', head: 'a1b2c3d4' }))).toBe(
			'Detached at a1b2c3d4',
		);
		expect(branchText(cleanSummary({ headKind: 'unborn' }))).toBe('main (no commits yet)');
	});

	it('describes a repository in a sentence a screen reader can read', () => {
		expect(summaryWords(cleanSummary(), 'waypoint')).toBe(
			'Git repository waypoint, on branch main. No changes',
		);
		const busy = cleanSummary({
			upstream: 'origin/main',
			ahead: 2,
			behind: 1,
			staged: 1,
			unstaged: 3,
			untracked: 2,
			conflicted: 1,
			operation: 'merge',
		});
		expect(summaryWords(busy, 'waypoint')).toBe(
			'Git repository waypoint, on branch main. 2 commits ahead of the upstream branch. 1 commit behind the upstream branch. A merge is in progress. 1 in conflict, 1 staged, 3 not staged, 2 untracked',
		);
		expect(
			summaryWords(cleanSummary({ upstream: 'origin/main', ahead: 0, behind: 0 }), 'r'),
		).toContain('Up to date with origin/main');
	});

	it('lists only the counts that are not zero', () => {
		expect(summaryCounts(cleanSummary())).toBe('');
		expect(summaryCounts(cleanSummary({ untracked: 4 }))).toBe('4 untracked');
	});
});

describe('which side speaks, and where a mark sorts', () => {
	it('lets the louder of the two sides speak, as Rust does', () => {
		expect(primaryChange({})).toBeNull();
		expect(primaryChange({ staged: 'modified', unstaged: 'deleted' })).toBe('deleted');
		expect(primaryChange({ staged: 'renamed', unstaged: 'modified' })).toBe('renamed');
		expect(primaryChange({ unstaged: 'conflicted' })).toBe('conflicted');
	});

	it('ranks conflicts first, then edits, new files, clean, then ignored', () => {
		const order: Array<GitMark | undefined> = [
			{ unstaged: 'conflicted' },
			{ inside: 2, conflictedInside: 1 },
			{ unstaged: 'modified' },
			{ unstaged: 'deleted' },
			{ staged: 'added' },
			{ staged: 'renamed' },
			{ unstaged: 'typeChanged' },
			{ unstaged: 'untracked' },
			{ inside: 2 },
			undefined,
			{ repository: true },
			{ unstaged: 'ignored' },
		];
		expect(order.map(markSortRank)).toEqual([0, 0, 1, 2, 3, 4, 5, 6, 6, 7, 7, 8]);
	});
});
