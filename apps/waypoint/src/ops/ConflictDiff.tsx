// A compact unified diff of two text files: added and removed lines with markers and line numbers, in words for a screen reader too
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TextDiff } from '@liminal-hq/waypoint-protocol/generated/TextDiff';
import { t, tf, type MessageId } from '../i18n/messages';
import styles from './ConflictDiff.module.css';
import { visibleDiff } from './conflictPreviewModel';
import { pluralText } from './resolveText';

interface ConflictDiffProps {
	id: string;
	/** The file's name, for the group's label. */
	name: string;
	diff: TextDiff;
}

const MARKS = { added: '+', removed: '−', context: ' ' } as const;

/**
 * The rows are plain lines in a labelled group. The marker and the numbers are for the eye only
 * (`aria-hidden`); each row starts with visually hidden words ("Added, line 4:") so the change is
 * never only a colour or a sign. A long diff stops after `DIFF_ROW_CAP` rows and says how many
 * lines it left out.
 */
export function ConflictDiff({ id, name, diff }: ConflictDiffProps) {
	const { lines, more } = visibleDiff(diff);
	return (
		<div
			id={id}
			className={styles.diff}
			role="group"
			aria-label={tf('ops.conflict.diff.label', { name })}
		>
			{lines.map((line, index) => {
				if (line.op === 'gap') {
					return (
						<div key={index} className={styles.gap} data-op="gap">
							{pluralText('ops.conflict.diff.gap', line.lines)}
						</div>
					);
				}
				const number = line.op === 'added' ? line.newLine : line.oldLine;
				return (
					<div key={index} className={styles.line} data-op={line.op}>
						<span className={styles.number} aria-hidden="true">
							{number}
						</span>
						<span className={styles.mark} aria-hidden="true">
							{MARKS[line.op]}
						</span>
						<span className={styles.srOnly}>
							{tf(`ops.conflict.diff.${line.op}` as MessageId, { line: number })}
						</span>
						<span className={styles.text}>{line.text}</span>
					</div>
				);
			})}
			{more > 0 && <div className={styles.more}>{pluralText('ops.conflict.diff.more', more)}</div>}
			{diff.lossy && <p className={styles.note}>{t('ops.conflict.preview.lossy')}</p>}
			{diff.approximate && <p className={styles.note}>{t('ops.conflict.preview.approximate')}</p>}
		</div>
	);
}
