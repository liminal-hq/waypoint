// Keyboard movement through a grouped listing: entries and collapsed headers are stops, open headers are stepped over
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupLayout } from './groupLayout';

/** Where the keyboard is: on an entry (by view position), on a group's header (by group index), or nowhere yet. */
export type Cursor = { readonly position: number } | { readonly header: number } | null;

/** Where a key takes it. */
export type Target = { readonly position: number } | { readonly header: number };

/** The column a cursor is in: an entry's place across its line, and the first column for a header. */
function columnOf(layout: GroupLayout, cursor: Cursor): number {
	if (cursor === null || !('position' in cursor)) return 0;
	const group = layout.groupOf(cursor.position);
	const start = group === null ? 0 : layout.groups[group]!.start;
	return (cursor.position - start) % layout.columns;
}

function rowOf(layout: GroupLayout, cursor: Cursor): number {
	if (cursor === null) return layout.firstStop();
	return 'header' in cursor ? layout.rowOfHeader(cursor.header) : layout.rowNear(cursor.position);
}

/** What stands at a row, in a column: the entry there (the line's last, if it is short) or the header. */
function targetAt(layout: GroupLayout, row: number, column: number): Target {
	const at = layout.rowAt(row);
	return at.kind === 'header'
		? { header: at.group }
		: { position: at.first + Math.min(column, at.count - 1) };
}

/**
 * Where a navigation key goes in a grouped listing, or `null` when it is not one this moves with (or
 * there is nowhere to go). Up, Down, Page Up, Page Down, Home and End move by rows and keep the
 * column; they step over the header of an open group and stop on the header of a collapsed one,
 * which has no entries to land on. Left and Right move one entry in a grid and skip collapsed groups;
 * Left from the first entry of a group (any entry, in a list) steps up onto the group's header, which
 * is how the keyboard reaches one to fold it.
 */
export function navigate(
	layout: GroupLayout,
	key: string,
	cursor: Cursor,
	pageRows: number,
): Target | null {
	if (layout.rowCount === 0) return null;
	const column = columnOf(layout, cursor);
	const first = layout.firstStop();
	const vertical = (rows: number): Target => {
		if (cursor === null) return targetAt(layout, first, 0);
		const from = rowOf(layout, cursor);
		const direction = rows >= 0 ? 1 : -1;
		const to = layout.snapToStop(from + rows, direction);
		// Nowhere further that way: the keyboard stays where it is.
		if (to === from || (direction === 1 ? to < from : to > from))
			return targetAt(layout, from, column);
		return targetAt(layout, to, column);
	};
	switch (key) {
		case 'ArrowDown':
			return vertical(1);
		case 'ArrowUp':
			return vertical(-1);
		case 'PageDown':
			return vertical(Math.max(1, pageRows));
		case 'PageUp':
			return vertical(-Math.max(1, pageRows));
		case 'Home':
			return targetAt(layout, first, 0);
		case 'End':
			return targetAt(layout, layout.lastStop(), layout.columns - 1);
		case 'ArrowRight': {
			if (layout.columns === 1) return null;
			if (cursor === null) return targetAt(layout, first, 0);
			if (!('position' in cursor)) return null;
			let next = cursor.position + 1;
			let group = layout.groupOf(next);
			while (group !== null && layout.isCollapsed(group)) {
				const run = layout.groups[group]!;
				next = run.start + run.count;
				group = layout.groupOf(next);
			}
			return next >= layout.count ? null : { position: next };
		}
		case 'ArrowLeft': {
			if (cursor === null) return targetAt(layout, first, 0);
			if (!('position' in cursor)) return null;
			const group = layout.groupOf(cursor.position);
			if (group === null)
				return layout.columns === 1 ? null : { position: Math.max(0, cursor.position - 1) };
			const atStart = cursor.position === layout.groups[group]!.start;
			if (layout.columns === 1 || atStart) return { header: group };
			return { position: cursor.position - 1 };
		}
		default:
			return null;
	}
}

/** Where the keyboard lands first in a listing: its first entry, or the header of a collapsed first group. */
export function firstTarget(layout: GroupLayout): Target {
	return layout.rowCount === 0 ? { position: 0 } : targetAt(layout, layout.firstStop(), 0);
}
