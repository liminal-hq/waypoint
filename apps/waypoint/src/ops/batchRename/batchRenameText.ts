// The words the batch rename dialog shows for what the preview found
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { BatchPreview } from '@liminal-hq/waypoint-protocol/generated/BatchPreview';
import type { PreviewRow } from '@liminal-hq/waypoint-protocol/generated/PreviewRow';
import type { Problem } from '@liminal-hq/waypoint-protocol/generated/Problem';
import { t, tf, tn } from '../../i18n/messages';
import { isBlocking } from './batchRenameModel';

/** What a problem says, in a sentence. `rows` is the whole preview, which a duplicate points into. */
export function problemText(problem: Problem, rows: readonly PreviewRow[]): string {
	switch (problem.kind) {
		case 'invalid':
			return tf('batchRename.problem.invalid', { reason: problem.reason });
		case 'duplicateTarget': {
			const other = rows.find((row) => row.index === problem.with);
			return tf('batchRename.problem.duplicate', { other: other?.to ?? '' });
		}
		case 'existsInFolder':
			return t('batchRename.problem.exists');
		case 'nestedSelection': {
			const outer = rows.find((row) => row.index === problem.with);
			return tf('batchRename.problem.nested', { other: outer?.from ?? '' });
		}
		case 'unchangedSkip':
			return t('batchRename.row.unchanged');
	}
}

/**
 * The note beside a row: each problem that stops the rename (named as one, so it never rests on
 * colour), or that the entry stays as it is, or that its extension changes.
 */
export function rowNotes(row: PreviewRow, rows: readonly PreviewRow[]): string[] {
	const notes = row.problems
		.filter(isBlocking)
		.map((problem) => `${t('batchRename.problem.label')}: ${problemText(problem, rows)}`);
	if (notes.length === 0 && !row.changed) notes.push(t('batchRename.row.unchanged'));
	if (row.extensionChanged) notes.push(t('batchRename.row.extension'));
	return notes;
}

/** The summary line: how many problems, or how many items will be renamed. */
export function summaryText(preview: BatchPreview): string {
	if (preview.problems > 0) return tn('batchRename.summary.problems', preview.problems);
	if (preview.changes === 0) return t('batchRename.summary.none');
	return tn('batchRename.summary.changes', preview.changes);
}
