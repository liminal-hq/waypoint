// Verifies the preview model: which clashes can be compared, the size comparison, and how much of a diff is drawn
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { DiffLine } from '@liminal-hq/waypoint-protocol/generated/DiffLine';
import type { TextDiff } from '@liminal-hq/waypoint-protocol/generated/TextDiff';
import { describe, expect, it } from 'vitest';
import { conflictFor } from '../test/opsHarness';
import { DIFF_ROW_CAP, canCompare, sizeHint, visibleDiff } from './conflictPreviewModel';

function diffOf(lines: DiffLine[], more = 0): TextDiff {
	return { lines, added: 0, removed: 0, more, approximate: false, lossy: false };
}

const added = (n: number): DiffLine => ({ op: 'added', newLine: n, text: `n${n}` });

describe('canCompare', () => {
	it('is true only for two files that both exist', () => {
		expect(canCompare(conflictFor('a'))).toBe(true);
		expect(canCompare(conflictFor('a', { kind: 'folderOverFolder' }))).toBe(false);
		expect(canCompare(conflictFor('a', { kind: 'fileOverFolder' }))).toBe(false);
		expect(canCompare(conflictFor('a', { withinBatch: true }))).toBe(false);
	});
});

describe('sizeHint', () => {
	it('says whether the incoming file is larger, smaller or the same size', () => {
		expect(sizeHint(conflictFor('a', { sourceSize: 2, existingSize: 1 }))).toBe('larger');
		expect(sizeHint(conflictFor('a', { sourceSize: 1, existingSize: 2 }))).toBe('smaller');
		expect(sizeHint(conflictFor('a', { sourceSize: 2, existingSize: 2 }))).toBe('same');
	});

	it('says nothing when a size is missing or the clash is not two files', () => {
		expect(sizeHint(conflictFor('a', { sourceSize: null }))).toBe('unknown');
		expect(sizeHint(conflictFor('a', { kind: 'folderOverFolder' }))).toBe('unknown');
	});
});

describe('visibleDiff', () => {
	it('draws every row of a short diff', () => {
		const diff = diffOf([added(1), added(2)]);
		expect(visibleDiff(diff)).toEqual({ lines: diff.lines, more: 0 });
	});

	it('stops at the cap and counts what was left, with what Rust left, without counting gaps', () => {
		const rows: DiffLine[] = Array.from({ length: DIFF_ROW_CAP }, (_, i) => added(i + 1));
		rows.push({ op: 'gap', lines: 9 }, added(300), added(301));
		const { lines, more } = visibleDiff(diffOf(rows, 5));
		expect(lines).toHaveLength(DIFF_ROW_CAP);
		expect(more).toBe(7);
	});
});
