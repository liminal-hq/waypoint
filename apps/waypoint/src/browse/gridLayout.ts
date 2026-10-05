// The grid's geometry: how many columns fit, how tall a row is, and how many items the scroll cap leaves
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupLayout } from './groupLayout';

/** Space around an item's icon, each side, in pixels. */
const CELL_PADDING = 12;
/** Room under the icon for a two-line name. */
const LABEL_HEIGHT = 40;
/** The grid's own padding, each side. */
export const GRID_PADDING = 12;
/** The height of a group's header between the rows of cells. */
export const GROUP_HEADER_HEIGHT = 36;

export interface GridCell {
	width: number;
	height: number;
}

/** The size of one cell for icons of `size` pixels. The stylesheet draws the same numbers from custom properties. */
export function cellFor(size: number): GridCell {
	return { width: size + CELL_PADDING * 2, height: size + CELL_PADDING * 2 + LABEL_HEIGHT };
}

/**
 * Whether the cell in `column` of row `rowIndex` has empty space under it: it is in the last row, or the
 * next row is a row of entries that stops short of that column. Such a cell can show its whole name
 * without covering another item. A group's header under a cell counts as something below it.
 */
export function hasRoomBelow(
	layout: Pick<GroupLayout, 'rowCount' | 'rowAt'>,
	rowIndex: number,
	column: number,
): boolean {
	if (rowIndex >= layout.rowCount - 1) return true;
	const next = layout.rowAt(rowIndex + 1);
	return next.kind === 'entries' && next.count <= column;
}

/** How many columns of `cell` fit in `width` pixels; always at least one. */
export function columnsFor(width: number, cell: GridCell): number {
	return Math.max(1, Math.floor((width - GRID_PADDING * 2) / cell.width));
}

/**
 * Where a navigation key goes in a grid of `columns`, from position `from`, with `last` the last
 * reachable position. Left and Right move one item, Up and Down one row (staying put at the top, and
 * keeping the column, or reaching the end of a short last row), Page keys move `pageRows` rows the
 * same way, and Home and End go to the ends. `null` for any other key.
 */
export function gridMove(
	key: string,
	from: number | null,
	last: number,
	columns: number,
	pageRows: number,
): number | null {
	const at = from ?? 0;
	const column = at % columns;
	const down = (rows: number) => {
		const target = at + rows * columns;
		if (target <= last) return target;
		// Past the last row: stay on the last row in the same column, or at its end when it is short.
		if (Math.floor(at / columns) >= Math.floor(last / columns)) return at;
		return Math.min(last, Math.floor(last / columns) * columns + column);
	};
	// Past the first row: stay in the first row, in the same column.
	const up = (rows: number) => (at - rows * columns >= 0 ? at - rows * columns : column);
	switch (key) {
		case 'ArrowRight':
			return from === null ? 0 : at + 1;
		case 'ArrowLeft':
			return from === null ? 0 : at - 1;
		case 'ArrowDown':
			return from === null ? 0 : down(1);
		case 'ArrowUp':
			return from === null ? 0 : up(1);
		case 'PageDown':
			return from === null ? 0 : down(pageRows);
		case 'PageUp':
			return from === null ? 0 : up(pageRows);
		case 'Home':
			return 0;
		case 'End':
			return last;
		default:
			return null;
	}
}
