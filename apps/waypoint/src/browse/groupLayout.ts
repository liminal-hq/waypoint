// The rows of a grouped listing: a header ahead of each group's entries, and what a collapsed group hides
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupKey } from '@liminal-hq/waypoint-protocol/generated/GroupKey';
import type { GroupRun } from '@liminal-hq/waypoint-protocol/generated/GroupRun';

/**
 * One row of what the view draws: a group's header, or a line of entries (one in a list, a row of
 * cells in a grid) from view position `first`, `count` of them.
 */
export type Row =
	| { readonly kind: 'header'; readonly group: number }
	| { readonly kind: 'entries'; readonly first: number; readonly count: number };

/** A stable name for a group, so a collapsed group stays collapsed as its rows come and go. */
export function groupId(key: GroupKey): string {
	return JSON.stringify(key);
}

/** The view positions `[start, end)` that collapsed groups hide, in order. */
export function hiddenRanges(
	groups: readonly GroupRun[],
	collapsed: ReadonlySet<string>,
): Array<[number, number]> {
	if (collapsed.size === 0) return [];
	return groups
		.filter((run) => collapsed.has(groupId(run.key)))
		.map((run): [number, number] => [run.start, run.start + run.count]);
}

/** `[start, end)` without the `hidden` ranges (which are sorted and apart): what is left, in order. */
export function withoutRanges(
	start: number,
	end: number,
	hidden: ReadonlyArray<readonly [number, number]>,
): Array<[number, number]> {
	const out: Array<[number, number]> = [];
	let at = start;
	for (const [from, to] of hidden) {
		if (to <= at) continue;
		if (from >= end) break;
		if (from > at) out.push([at, Math.min(from, end)]);
		at = Math.max(at, to);
	}
	if (at < end) out.push([at, end]);
	return out;
}

/**
 * The display rows of a listing: with groups, each group's header and then its entries (none, for a
 * collapsed group), `columns` to a line; without, just the lines of entries. Rows are worked out by
 * arithmetic over the groups, so a half-million-entry listing builds nothing per entry.
 *
 * Headers are not entries: they are never selected, and the keyboard steps over them. A collapsed
 * group has no entry rows, so its header is where the keyboard can stop to expand it again.
 */
export class GroupLayout {
	readonly grouped: boolean;
	readonly rowCount: number;
	/** The display row of each group's header. */
	private readonly headerRows: number[] = [];

	constructor(
		readonly groups: readonly GroupRun[],
		readonly collapsed: ReadonlySet<string>,
		readonly count: number,
		readonly columns = 1,
	) {
		this.grouped = groups.length > 0;
		let row = 0;
		for (const run of groups) {
			this.headerRows.push(row);
			row += 1 + (this.isCollapsedRun(run) ? 0 : this.lines(run.count));
		}
		this.rowCount = this.grouped ? row : this.lines(count);
	}

	private lines(entries: number): number {
		return Math.ceil(entries / this.columns);
	}

	private isCollapsedRun(run: GroupRun): boolean {
		return this.collapsed.has(groupId(run.key));
	}

	isCollapsed(group: number): boolean {
		const run = this.groups[group];
		return run !== undefined && this.isCollapsedRun(run);
	}

	/** The index of the group whose header is at or before `row`. */
	private groupAtRow(row: number): number {
		let low = 0;
		let high = this.headerRows.length - 1;
		while (low < high) {
			const middle = (low + high + 1) >> 1;
			if (this.headerRows[middle]! <= row) low = middle;
			else high = middle - 1;
		}
		return low;
	}

	rowAt(row: number): Row {
		if (!this.grouped) {
			const first = row * this.columns;
			return { kind: 'entries', first, count: Math.min(this.columns, this.count - first) };
		}
		const group = this.groupAtRow(row);
		const line = row - this.headerRows[group]! - 1;
		if (line < 0) return { kind: 'header', group };
		const run = this.groups[group]!;
		const first = run.start + line * this.columns;
		return { kind: 'entries', first, count: Math.min(this.columns, run.start + run.count - first) };
	}

	/** The index of the group that holds a view position, or `null` where the listing is not grouped. */
	groupOf(position: number): number | null {
		if (!this.grouped) return null;
		let low = 0;
		let high = this.groups.length - 1;
		while (low < high) {
			const middle = (low + high + 1) >> 1;
			if (this.groups[middle]!.start <= position) low = middle;
			else high = middle - 1;
		}
		return low;
	}

	/** The display row of an entry, or `null` when its group is collapsed (it has no row). */
	rowOfEntry(position: number): number | null {
		const group = this.groupOf(position);
		if (group === null) return Math.floor(position / this.columns);
		if (this.isCollapsed(group)) return null;
		const offset = position - this.groups[group]!.start;
		return this.headerRows[group]! + 1 + Math.floor(offset / this.columns);
	}

	rowOfHeader(group: number): number {
		return this.headerRows[group] ?? 0;
	}

	/** The display row to draw for a position even in a collapsed group, whose header it is then. */
	rowNear(position: number): number {
		const row = this.rowOfEntry(position);
		if (row !== null) return row;
		const group = this.groupOf(position);
		return group === null ? 0 : this.rowOfHeader(group);
	}

	/** The view position at the start of a row, for following the row at the top of the viewport through a patch. */
	positionAtRow(row: number): number {
		if (this.rowCount === 0) return 0;
		const at = this.rowAt(Math.max(0, Math.min(this.rowCount - 1, row)));
		return at.kind === 'entries' ? at.first : (this.groups[at.group]?.start ?? 0);
	}

	/** How many headers sit before a row. */
	private headersBefore(row: number): number {
		if (!this.grouped || row <= 0) return 0;
		let low = 0;
		let high = this.headerRows.length;
		while (low < high) {
			const middle = (low + high) >> 1;
			if (this.headerRows[middle]! < row) low = middle + 1;
			else high = middle;
		}
		return low;
	}

	/** How far down the scroll content a row starts, with headers `headerSize` tall and lines `lineSize`. */
	offsetOfRow(row: number, headerSize: number, lineSize: number): number {
		const headers = this.headersBefore(row);
		return headers * headerSize + (row - headers) * lineSize;
	}

	/** The row at a scroll offset: the last one that starts at or before it. */
	rowAtOffset(offset: number, headerSize: number, lineSize: number): number {
		let low = 0;
		let high = Math.max(0, this.rowCount - 1);
		while (low < high) {
			const middle = (low + high + 1) >> 1;
			if (this.offsetOfRow(middle, headerSize, lineSize) <= offset) low = middle;
			else high = middle - 1;
		}
		return low;
	}

	/** How many entries sit in the first `rows` rows (all of them when the rows reach the end). */
	entriesWithin(rows: number): number {
		if (rows >= this.rowCount) return this.count;
		if (rows <= 0) return 0;
		const last = this.rowAt(rows - 1);
		if (last.kind === 'entries') return last.first + last.count;
		return this.groups[last.group]?.start ?? 0;
	}

	/** Whether the keyboard can land on a row: a line of entries, or the header of a collapsed group. */
	isStop(row: number): boolean {
		const at = this.rowAt(row);
		return at.kind === 'entries' || this.isCollapsed(at.group);
	}

	/** The row at or beyond `row` (in `direction`) that the keyboard can land on, kept inside the rows. */
	snapToStop(row: number, direction: 1 | -1): number {
		const last = this.rowCount - 1;
		const from = Math.max(0, Math.min(last, row));
		let at = from;
		while (at >= 0 && at <= last && !this.isStop(at)) at += direction;
		if (at >= 0 && at <= last) return at;
		// Nothing that way (an opening header at the very top): the other way always has the last row.
		return this.snapToStop(from, direction === 1 ? -1 : 1);
	}

	/** The first row the keyboard can land on. */
	firstStop(): number {
		return this.rowCount === 0 ? 0 : this.snapToStop(0, 1);
	}

	/** The last row the keyboard can land on. */
	lastStop(): number {
		return this.rowCount === 0 ? 0 : this.snapToStop(this.rowCount - 1, -1);
	}
}
