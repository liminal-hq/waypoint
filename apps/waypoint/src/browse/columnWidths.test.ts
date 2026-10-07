// Verifies the column width model: limits, drag and key arithmetic, and that the stylesheet declares the same defaults
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	COLUMN_WIDTH_MAX,
	COLUMN_WIDTH_MIN,
	type ListColumnWidths,
} from '../services/folderViewsClient';
import {
	COLUMN_LIMITS,
	KEY_BIG_STEP,
	KEY_STEP,
	RESIZABLE_COLUMNS,
	clampWidth,
	isResizable,
	noWidths,
	resolveWidths,
	widestWidth,
	widthForDrag,
	widthForKey,
	widthProperties,
	widthVariable,
	withWidth,
} from './columnWidths';

describe('the limits', () => {
	it('sit inside the bounds Rust enforces, with the initial width between them', () => {
		for (const column of RESIZABLE_COLUMNS) {
			const { initial, min, max } = COLUMN_LIMITS[column];
			expect(min, column).toBeGreaterThanOrEqual(COLUMN_WIDTH_MIN);
			expect(max, column).toBeLessThanOrEqual(COLUMN_WIDTH_MAX);
			expect(initial, column).toBeGreaterThanOrEqual(min);
			expect(initial, column).toBeLessThanOrEqual(max);
		}
	});

	it('are the ones the stylesheet declares as each column’s own width and shrink limit', () => {
		const css = readFileSync(resolve(process.cwd(), 'src/browse/ListView.module.css'), 'utf8');
		for (const column of RESIZABLE_COLUMNS) {
			const name = widthVariable(column);
			const { initial, min } = COLUMN_LIMITS[column];
			expect(css, `${name}-min`).toMatch(new RegExp(`${name}-min:\\s*${min}px;`));
			// The Trash's original location shares the room with Name until it is resized.
			if (column === 'original') expect(css).toMatch(new RegExp(`${name}:\\s*1fr;`));
			else expect(css, name).toMatch(new RegExp(`${name}:\\s*${initial}px;`));
		}
	});

	it('name every column Rust stores, and nothing else', () => {
		// `satisfies` makes the compiler hold this list to the generated type's keys.
		const stored = {
			size: true,
			modified: true,
			kind: true,
			git: true,
			storageClass: true,
			original: true,
			deleted: true,
		} satisfies Record<keyof ListColumnWidths, true>;
		expect([...RESIZABLE_COLUMNS].sort()).toEqual(Object.keys(stored).sort());
		expect(isResizable('size')).toBe(true);
		expect(isResizable('name')).toBe(false);
		expect(isResizable('toString')).toBe(false);
	});
});

describe('clamping', () => {
	it('keeps a width whole and between the column’s minimum and maximum', () => {
		expect(clampWidth('size', 100.4)).toBe(100);
		expect(clampWidth('size', 1)).toBe(COLUMN_LIMITS.size.min);
		expect(clampWidth('size', 9999)).toBe(COLUMN_LIMITS.size.max);
	});

	it('stops a column growing past the room Name can give', () => {
		expect(clampWidth('modified', 300, 40, 168)).toBe(208);
		expect(widestWidth('modified', 168, 40)).toBe(208);
		expect(widestWidth('modified', 168, 1000)).toBe(COLUMN_LIMITS.modified.max);
	});

	it('never forces a column narrower because the room has run out, and lets it shrink', () => {
		expect(clampWidth('modified', 200, -50, 168)).toBe(168);
		expect(clampWidth('modified', 120, -50, 168)).toBe(120);
		expect(widestWidth('modified', 168, -50)).toBe(168);
	});

	it('brings stored widths to the limits and leaves out the columns that were not resized', () => {
		const stored = { ...noWidths(), size: 5000, git: 50, kind: null };
		expect(resolveWidths(stored)).toEqual({ size: COLUMN_LIMITS.size.max, git: 50 });
		expect(resolveWidths(null)).toEqual({});
		expect(resolveWidths(undefined)).toEqual({});
	});
});

describe('the style', () => {
	it('names a variable per column in kebab case', () => {
		expect(widthVariable('size')).toBe('--wp-col-size');
		expect(widthVariable('storageClass')).toBe('--wp-col-storage-class');
	});

	it('sets only the columns that were resized', () => {
		expect(widthProperties({ size: 120, storageClass: 140 })).toEqual({
			'--wp-col-size': '120px',
			'--wp-col-storage-class': '140px',
		});
		expect(widthProperties({})).toEqual({});
	});

	it('changes one stored width without touching the others', () => {
		const stored = { ...noWidths(), size: 120 };
		expect(withWidth(stored, 'kind', 130)).toEqual({ ...stored, kind: 130 });
		expect(withWidth(stored, 'size', null)).toEqual(noWidths());
	});
});

describe('a drag', () => {
	it('widens the column as the divider moves toward the start edge, and narrows it the other way', () => {
		expect(widthForDrag('size', 88, -30, { rtl: false })).toBe(118);
		expect(widthForDrag('size', 88, 12, { rtl: false })).toBe(76);
		expect(widthForDrag('size', 88, 30, { rtl: true })).toBe(118);
		expect(widthForDrag('size', 88, -12, { rtl: true })).toBe(76);
	});

	it('stops at the column’s limits and at Name’s minimum', () => {
		expect(widthForDrag('size', 88, 500, { rtl: false })).toBe(COLUMN_LIMITS.size.min);
		expect(widthForDrag('size', 88, -500, { rtl: false })).toBe(COLUMN_LIMITS.size.max);
		expect(widthForDrag('size', 88, -500, { rtl: false, room: 20 })).toBe(108);
	});
});

describe('the keys', () => {
	const options = { rtl: false, shift: false };

	it('widen toward the start edge by a step, and by a larger one with Shift', () => {
		expect(widthForKey('size', 88, 'ArrowLeft', options)).toEqual({
			kind: 'set',
			width: 88 + KEY_STEP,
		});
		expect(widthForKey('size', 88, 'ArrowRight', options)).toEqual({
			kind: 'set',
			width: 88 - KEY_STEP,
		});
		expect(widthForKey('size', 88, 'ArrowLeft', { ...options, shift: true })).toEqual({
			kind: 'set',
			width: 88 + KEY_BIG_STEP,
		});
	});

	it('mirror in a right-to-left view', () => {
		expect(widthForKey('size', 88, 'ArrowRight', { ...options, rtl: true })).toEqual({
			kind: 'set',
			width: 88 + KEY_STEP,
		});
		expect(widthForKey('size', 88, 'ArrowLeft', { ...options, rtl: true })).toEqual({
			kind: 'set',
			width: 88 - KEY_STEP,
		});
	});

	it('go to the limits with Home and End, and restore the column’s own width with Backspace or Delete', () => {
		expect(widthForKey('size', 88, 'Home', options)).toEqual({
			kind: 'set',
			width: COLUMN_LIMITS.size.min,
		});
		expect(widthForKey('size', 88, 'End', options)).toEqual({
			kind: 'set',
			width: COLUMN_LIMITS.size.max,
		});
		expect(widthForKey('size', 88, 'End', { ...options, room: 10 })).toEqual({
			kind: 'set',
			width: 98,
		});
		expect(widthForKey('size', 88, 'Backspace', options)).toEqual({ kind: 'reset' });
		expect(widthForKey('size', 88, 'Delete', options)).toEqual({ kind: 'reset' });
	});

	it('leave other keys alone', () => {
		expect(widthForKey('size', 88, 'ArrowUp', options)).toBeNull();
		expect(widthForKey('size', 88, 'Tab', options)).toBeNull();
	});

	it('stop at the limits', () => {
		expect(widthForKey('size', COLUMN_LIMITS.size.min, 'ArrowRight', options)).toEqual({
			kind: 'set',
			width: COLUMN_LIMITS.size.min,
		});
		expect(widthForKey('size', COLUMN_LIMITS.size.max, 'ArrowLeft', options)).toEqual({
			kind: 'set',
			width: COLUMN_LIMITS.size.max,
		});
	});
});
