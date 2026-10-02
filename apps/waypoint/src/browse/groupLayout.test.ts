// Verifies the rows of a grouped listing: headers among the entries, folded groups, offsets and keyboard stops
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupRun } from '@liminal-hq/waypoint-protocol/generated/GroupRun';
import { describe, expect, it } from 'vitest';
import { GroupLayout, groupId, hiddenRanges, withoutRanges } from './groupLayout';

const run = (extension: string, start: number, count: number): GroupRun => ({
	key: { kind: 'type', extension },
	start,
	count,
});

// Three groups over ten entries: 0-2, 3-4 and 5-9.
const GROUPS = [run('a', 0, 3), run('b', 3, 2), run('c', 5, 5)];
const fold = (...runs: GroupRun[]) => new Set(runs.map((r) => groupId(r.key)));

describe('a list of headers and entries', () => {
	const layout = new GroupLayout(GROUPS, new Set(), 10);

	it('puts a header before each group, so the rows are the entries and the headers', () => {
		expect(layout.rowCount).toBe(13);
		expect(layout.rowAt(0)).toEqual({ kind: 'header', group: 0 });
		expect(layout.rowAt(1)).toEqual({ kind: 'entries', first: 0, count: 1 });
		expect(layout.rowAt(4)).toEqual({ kind: 'header', group: 1 });
		expect(layout.rowAt(5)).toEqual({ kind: 'entries', first: 3, count: 1 });
		expect(layout.rowAt(7)).toEqual({ kind: 'header', group: 2 });
		expect(layout.rowAt(12)).toEqual({ kind: 'entries', first: 9, count: 1 });
	});

	it('finds the row of an entry and of a header, and the group of a position', () => {
		expect(layout.rowOfEntry(0)).toBe(1);
		expect(layout.rowOfEntry(3)).toBe(5);
		expect(layout.rowOfEntry(9)).toBe(12);
		expect(layout.rowOfHeader(2)).toBe(7);
		expect(layout.groupOf(4)).toBe(1);
		expect(layout.groupOf(5)).toBe(2);
	});

	it('is just the entries when there are no groups', () => {
		const plain = new GroupLayout([], new Set(), 10);
		expect(plain.grouped).toBe(false);
		expect(plain.rowCount).toBe(10);
		expect(plain.rowAt(4)).toEqual({ kind: 'entries', first: 4, count: 1 });
		expect(plain.rowOfEntry(4)).toBe(4);
		expect(plain.groupOf(4)).toBeNull();
	});
});

describe('a folded group', () => {
	const layout = new GroupLayout(GROUPS, fold(GROUPS[1]!), 10);

	it('keeps its header and loses its rows', () => {
		expect(layout.rowCount).toBe(11);
		expect(layout.rowAt(4)).toEqual({ kind: 'header', group: 1 });
		expect(layout.rowAt(5)).toEqual({ kind: 'header', group: 2 });
		expect(layout.rowOfEntry(3)).toBeNull();
		expect(layout.rowOfEntry(5)).toBe(6);
		expect(layout.rowNear(3)).toBe(4);
		expect(layout.isCollapsed(1)).toBe(true);
	});

	it('names the positions it hides, and takes them out of a range', () => {
		expect(hiddenRanges(GROUPS, fold(GROUPS[1]!, GROUPS[2]!))).toEqual([
			[3, 5],
			[5, 10],
		]);
		expect(hiddenRanges(GROUPS, new Set())).toEqual([]);
		expect(withoutRanges(1, 8, [[3, 5]])).toEqual([
			[1, 3],
			[5, 8],
		]);
		expect(withoutRanges(3, 5, [[3, 5]])).toEqual([]);
		expect(withoutRanges(0, 10, [])).toEqual([[0, 10]]);
	});

	it('counts the entries within the first rows', () => {
		expect(layout.entriesWithin(11)).toBe(10);
		expect(layout.entriesWithin(5)).toBe(3);
		expect(layout.entriesWithin(6)).toBe(5);
	});
});

describe('keyboard stops', () => {
	const open = new GroupLayout(GROUPS, new Set(), 10);
	const folded = new GroupLayout(GROUPS, fold(GROUPS[1]!), 10);

	it('steps over an open header and lands on a folded one', () => {
		expect(open.isStop(0)).toBe(false);
		expect(open.isStop(1)).toBe(true);
		expect(folded.isStop(4)).toBe(true);
		expect(open.firstStop()).toBe(1);
		expect(open.lastStop()).toBe(12);
		expect(open.snapToStop(4, 1)).toBe(5);
		expect(open.snapToStop(4, -1)).toBe(3);
		// Nothing above the first header: the way back down is the first stop.
		expect(open.snapToStop(0, -1)).toBe(1);
	});

	it('ends on a folded last group', () => {
		const last = new GroupLayout(GROUPS, fold(GROUPS[2]!), 10);
		expect(last.lastStop()).toBe(last.rowOfHeader(2));
	});
});

describe('offsets with taller or shorter headers', () => {
	const layout = new GroupLayout(GROUPS, new Set(), 10);

	it('adds up the headers and the lines before a row', () => {
		expect(layout.offsetOfRow(0, 36, 28)).toBe(0);
		expect(layout.offsetOfRow(1, 36, 28)).toBe(36);
		expect(layout.offsetOfRow(5, 36, 28)).toBe(2 * 36 + 3 * 28);
		expect(layout.offsetOfRow(12, 36, 28)).toBe(3 * 36 + 9 * 28);
	});

	it('finds the row at an offset, the inverse of that', () => {
		for (let row = 0; row < layout.rowCount; row++) {
			const offset = layout.offsetOfRow(row, 36, 28);
			expect(layout.rowAtOffset(offset, 36, 28)).toBe(row);
			expect(layout.rowAtOffset(offset + 5, 36, 28)).toBe(row);
		}
	});

	it('names the position at the start of a row, the first of a group for its header', () => {
		expect(layout.positionAtRow(4)).toBe(3);
		expect(layout.positionAtRow(6)).toBe(4);
	});
});

describe('rows of cells in a grid', () => {
	// Groups of 3, 2 and 5 in rows of four: one, one and two lines.
	const layout = new GroupLayout(GROUPS, new Set(), 10, 4);

	it('lays each group out in its own lines of up to `columns` cells', () => {
		expect(layout.rowCount).toBe(3 + 1 + 1 + 2);
		expect(layout.rowAt(1)).toEqual({ kind: 'entries', first: 0, count: 3 });
		expect(layout.rowAt(3)).toEqual({ kind: 'entries', first: 3, count: 2 });
		expect(layout.rowAt(5)).toEqual({ kind: 'entries', first: 5, count: 4 });
		expect(layout.rowAt(6)).toEqual({ kind: 'entries', first: 9, count: 1 });
		expect(layout.rowOfEntry(8)).toBe(5);
		expect(layout.rowOfEntry(9)).toBe(6);
	});
});
