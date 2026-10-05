// What Git's marks and summaries say, in words and letters: pure, so every screen says the same thing
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GitChange } from '@liminal-hq/waypoint-protocol/generated/GitChange';
import type { GitMark } from '@liminal-hq/waypoint-protocol/generated/GitMark';
import type { GitSummary } from '@liminal-hq/waypoint-protocol/generated/GitSummary';
import { t, tf, tn, type MessageId } from '../i18n/messages';

/** The letter `git status --short` uses for each kind of change; never the only way a change is shown. */
export const CHANGE_LETTERS: Record<GitChange, string> = {
	modified: 'M',
	added: 'A',
	deleted: 'D',
	renamed: 'R',
	typeChanged: 'T',
	untracked: '?',
	ignored: '!',
	conflicted: 'U',
};

const CHANGE_WORDS: Record<GitChange, MessageId> = {
	modified: 'git.change.modified',
	added: 'git.change.added',
	deleted: 'git.change.deleted',
	renamed: 'git.change.renamed',
	typeChanged: 'git.change.typeChanged',
	untracked: 'git.change.untracked',
	ignored: 'git.change.ignored',
	conflicted: 'git.change.conflicted',
};

/** Untracked, ignored and conflicted paths are not staged or unstaged: they are what they are. */
const SIDELESS: readonly GitChange[] = ['untracked', 'ignored', 'conflicted'];

/** The change a side holds in words: "Modified, staged", "Added, not staged", or just "Untracked". */
function sideWords(change: GitChange, staged: boolean): string {
	const word = t(CHANGE_WORDS[change]);
	if (SIDELESS.includes(change)) return word;
	return tf(staged ? 'git.side.staged' : 'git.side.unstaged', { change: word });
}

/**
 * What a screen reader hears for a mark: both sides of a change ("Modified, staged; Modified, not
 * staged"), whether a folder is a repository, and its count of changes inside.
 */
export function markWords(mark: GitMark): string {
	const parts: string[] = [];
	if (mark.staged) parts.push(sideWords(mark.staged, true));
	if (mark.unstaged) parts.push(sideWords(mark.unstaged, false));
	if (mark.repository) parts.push(t('git.mark.repository'));
	const inside = mark.inside ?? 0;
	if (inside > 0) {
		parts.push(tn('git.mark.inside', inside));
		const conflicts = mark.conflictedInside ?? 0;
		if (conflicts > 0) parts.push(tn('git.mark.insideConflicts', conflicts));
	}
	return parts.join('; ');
}

/** The two letters of the Git column: the staged side, then the one not staged, either blank. */
export function markLetters(mark: GitMark): { staged: string; unstaged: string } {
	return {
		staged: mark.staged ? CHANGE_LETTERS[mark.staged] : '',
		unstaged: mark.unstaged ? CHANGE_LETTERS[mark.unstaged] : '',
	};
}

/** Whether a mark has a change to show in the Git column (a plain "this is a repository" has none). */
export function hasStatus(mark: GitMark | undefined): mark is GitMark {
	return (
		mark !== undefined && (mark.staged != null || mark.unstaged != null || (mark.inside ?? 0) > 0)
	);
}

/** The branch item's text: the branch or short commit, with the distance from the upstream. */
export function branchText(summary: GitSummary): string {
	const base =
		summary.headKind === 'detached'
			? tf('git.head.detached', { commit: summary.head })
			: summary.headKind === 'unborn'
				? tf('git.head.unborn', { branch: summary.head })
				: summary.head;
	const distance: string[] = [];
	const cap = summary.distanceCapped ? '+' : '';
	if (summary.ahead) distance.push(`↑${summary.ahead}${cap}`);
	if (summary.behind) distance.push(`↓${summary.behind}${cap}`);
	return [base, ...distance].join(' ');
}

/** How the repository stands, in words: what a screen reader and a tooltip say about the branch item. */
export function summaryWords(summary: GitSummary, repository: string): string {
	const parts: string[] = [];
	parts.push(
		summary.headKind === 'detached'
			? tf('git.words.detached', { commit: summary.head, repository })
			: summary.headKind === 'unborn'
				? tf('git.words.unborn', { branch: summary.head, repository })
				: tf('git.words.branch', { branch: summary.head, repository }),
	);
	if (summary.upstream) {
		if (!summary.ahead && !summary.behind) {
			parts.push(tf('git.words.upToDate', { upstream: summary.upstream }));
		} else {
			if (summary.ahead) parts.push(tn('git.words.ahead', summary.ahead));
			if (summary.behind) parts.push(tn('git.words.behind', summary.behind));
		}
	}
	if (summary.operation) parts.push(t(`git.operation.${summary.operation}` as MessageId));
	const counts = summaryCounts(summary);
	parts.push(counts === '' ? t('git.words.clean') : counts);
	return parts.join('. ');
}

/** "2 staged, 1 not staged, 3 untracked, 1 in conflict": the parts that are not zero; empty when clean. */
export function summaryCounts(summary: GitSummary): string {
	const parts: string[] = [];
	if (summary.conflicted > 0) parts.push(tn('git.count.conflicted', summary.conflicted));
	if (summary.staged > 0) parts.push(tn('git.count.staged', summary.staged));
	if (summary.unstaged > 0) parts.push(tn('git.count.unstaged', summary.unstaged));
	if (summary.untracked > 0) parts.push(tn('git.count.untracked', summary.untracked));
	return parts.join(', ');
}
