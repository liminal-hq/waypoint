// The list's column widths: each column's limits, the width a drag or key press asks for, and the style that carries them to the grid
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ListColumnWidths } from '../services/settingsClient';

/** A column whose width the person can change. Name is not one: it takes what the others leave. */
export type ResizableColumn = keyof ListColumnWidths;

/** What a column may be made, in pixels. The limits keep its heading readable and stop one column from taking the whole view. */
export interface ColumnLimits {
	/** The width the stylesheet gives the column until it is resized. The Trash's original location has none (it shares the room with Name), so this is only the width a screen reader is told before it is measured. */
	initial: number;
	min: number;
	max: number;
}

/** The stylesheet declares the same `initial` and `min` for each column (`--wp-col-*`); `columnWidths.test.ts` keeps the two in step. */
export const COLUMN_LIMITS: Record<ResizableColumn, ColumnLimits> = {
	size: { initial: 88, min: 64, max: 240 },
	modified: { initial: 168, min: 96, max: 360 },
	kind: { initial: 96, min: 64, max: 240 },
	git: { initial: 56, min: 40, max: 160 },
	storageClass: { initial: 128, min: 96, max: 360 },
	original: { initial: 240, min: 120, max: 800 },
	deleted: { initial: 168, min: 96, max: 360 },
};

/** The columns, in the order the header lists them. */
export const RESIZABLE_COLUMNS = Object.keys(COLUMN_LIMITS) as ResizableColumn[];

/** A key press moves a column's edge this far, and with Shift this far. */
export const KEY_STEP = 8;
export const KEY_BIG_STEP = 32;

/** The widths in force: only the columns that have been resized. */
export type ColumnWidths = Partial<Record<ResizableColumn, number>>;

export function isResizable(id: string): id is ResizableColumn {
	return Object.hasOwn(COLUMN_LIMITS, id);
}

/**
 * `width` made whole and kept between the column's minimum and its maximum, and also under
 * `room` more than `from` when the room left by Name is given: a drag that would squeeze Name
 * below its own minimum stops there. A column already wider than that (a pane that has narrowed
 * since) is never forced narrower by the room, only by its own limits.
 */
export function clampWidth(
	column: ResizableColumn,
	width: number,
	room = Number.POSITIVE_INFINITY,
	from = width,
): number {
	const { min, max } = COLUMN_LIMITS[column];
	const ceiling = Math.max(min, Math.min(max, Math.max(from, from + room)));
	return Math.round(Math.min(ceiling, Math.max(min, width)));
}

/** The widest `column` may now be, given where it starts and how much room Name has to give. */
export function widestWidth(column: ResizableColumn, from: number, room: number): number {
	return clampWidth(column, COLUMN_LIMITS[column].max, room, from);
}

/** The stored widths as the list uses them: the columns with one, each kept to its limits. */
export function resolveWidths(stored: ListColumnWidths): ColumnWidths {
	const widths: ColumnWidths = {};
	for (const column of RESIZABLE_COLUMNS) {
		const width = stored[column];
		if (width !== null && Number.isFinite(width)) widths[column] = clampWidth(column, width);
	}
	return widths;
}

/** The custom property the stylesheet reads for `column`'s width. */
export function widthVariable(column: ResizableColumn): string {
	return `--wp-col-${column.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`;
}

/** The custom properties that carry the widths to the header and every row; a column not resized keeps the stylesheet's own. */
export function widthProperties(widths: ColumnWidths): Record<string, string> {
	const properties: Record<string, string> = {};
	for (const column of RESIZABLE_COLUMNS) {
		const width = widths[column];
		if (width !== undefined) properties[widthVariable(column)] = `${width}px`;
	}
	return properties;
}

/** The stored widths with `column` set to `width`, or back to its own width for `null`. */
export function withWidth(
	stored: ListColumnWidths,
	column: ResizableColumn,
	width: number | null,
): ListColumnWidths {
	return { ...stored, [column]: width };
}

/** Every column back to its own width. */
export function noWidths(): ListColumnWidths {
	return Object.fromEntries(RESIZABLE_COLUMNS.map((column) => [column, null])) as ListColumnWidths;
}

/** What a key does to a column that is `current` pixels wide. */
export type WidthKey =
	| { kind: 'set'; width: number }
	/** Back to the column's own width. */
	| { kind: 'reset' };

/**
 * The width a key on a column's divider asks for, or `null` for a key it does not handle. The
 * divider is the column's start edge, so the key that moves it toward the start widens the
 * column: Left in a left-to-right view and Right in a right-to-left one. Home and End are the
 * narrowest and the widest the column may be; Backspace and Delete restore its own width.
 */
export function widthForKey(
	column: ResizableColumn,
	current: number,
	key: string,
	options: { rtl: boolean; shift: boolean; room?: number },
): WidthKey | null {
	const { min } = COLUMN_LIMITS[column];
	const step = options.shift ? KEY_BIG_STEP : KEY_STEP;
	const wider = options.rtl ? 'ArrowRight' : 'ArrowLeft';
	const narrower = options.rtl ? 'ArrowLeft' : 'ArrowRight';
	const room = options.room ?? Number.POSITIVE_INFINITY;
	switch (key) {
		case wider:
			return { kind: 'set', width: clampWidth(column, current + step, room, current) };
		case narrower:
			return { kind: 'set', width: clampWidth(column, current - step, room, current) };
		case 'Home':
			return { kind: 'set', width: min };
		case 'End':
			return { kind: 'set', width: widestWidth(column, current, room) };
		case 'Backspace':
		case 'Delete':
			return { kind: 'reset' };
		default:
			return null;
	}
}

/** The width a drag asks for once the pointer has moved `distance` pixels (positive is to the right) from where it went down. */
export function widthForDrag(
	column: ResizableColumn,
	from: number,
	distance: number,
	options: { rtl: boolean; room?: number },
): number {
	const grown = options.rtl ? distance : -distance;
	return clampWidth(column, from + grown, options.room ?? Number.POSITIVE_INFINITY, from);
}
