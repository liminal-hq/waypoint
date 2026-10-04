// What Git says about an entry, drawn: letters and a dot with the words beside them for assistive technology
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GitMark } from '@liminal-hq/waypoint-protocol/generated/GitMark';
import styles from './GitMarkView.module.css';
import { hasStatus, markLetters, markWords } from './gitModel';
import { primaryChange } from './markRank';

interface GitMarkViewProps {
	mark: GitMark | undefined;
	/** `column` shows the staged and unstaged letters; `chip` is one small mark beside an icon or a name. */
	variant: 'column' | 'chip';
}

/**
 * The letters of a change (never colour alone: the colour only repeats them), a dot for a folder with
 * changes inside, and the same in words for a screen reader and a tooltip. A repository with
 * nothing to report says "Git repository" in words only: the icon carries the sticker.
 */
export function GitMarkView({ mark, variant }: GitMarkViewProps) {
	if (!mark) return null;
	const words = markWords(mark);
	if (words === '') return null;
	if (!hasStatus(mark)) {
		return <span className={styles.srOnly}>{words}</span>;
	}
	const letters = markLetters(mark);
	const change = primaryChange(mark);
	const dot = (mark.inside ?? 0) > 0;
	const conflicted = change === 'conflicted' || (mark.conflictedInside ?? 0) > 0;
	return (
		<span
			className={styles.mark}
			data-variant={variant}
			data-change={conflicted ? 'conflicted' : (change ?? 'inside')}
			title={words}
		>
			<span aria-hidden="true" className={styles.letters}>
				{variant === 'column' ? (
					<>
						<span className={styles.side}>{letters.staged}</span>
						<span className={styles.side}>{letters.unstaged}</span>
					</>
				) : (
					<span className={styles.side}>{letters.unstaged || letters.staged}</span>
				)}
				{dot && <span className={styles.dot}>●</span>}
			</span>
			<span className={styles.srOnly}>{words}</span>
		</span>
	);
}
