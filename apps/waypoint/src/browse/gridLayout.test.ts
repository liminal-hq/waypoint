// Verifies the grid's geometry: columns, the scroll cap in rows of columns, and 2-D keyboard movement
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { cellFor, columnsFor, gridMove, GRID_PADDING } from './gridLayout';
import { GroupLayout } from './groupLayout';
import { MAX_SCROLL_HEIGHT, visibleRows } from './scrollCap';

describe('cellFor and columnsFor', () => {
	it('makes a cell the icon, its padding and a label', () => {
		expect(cellFor(96)).toEqual({ width: 120, height: 160 });
		expect(cellFor(48).width).toBe(72);
	});

	it('fits as many whole columns as the width allows, and always one', () => {
		const cell = cellFor(96);
		expect(columnsFor(GRID_PADDING * 2 + 120 * 5, cell)).toBe(5);
		expect(columnsFor(GRID_PADDING * 2 + 120 * 5 - 1, cell)).toBe(4);
		expect(columnsFor(50, cell)).toBe(1);
		expect(columnsFor(0, cell)).toBe(1);
	});

	it('gives more columns to smaller icons', () => {
		expect(columnsFor(800, cellFor(48))).toBeGreaterThan(columnsFor(800, cellFor(256)));
	});
});

describe('the scroll cap in rows of columns', () => {
	const capped = (count: number, columns: number, cell: ReturnType<typeof cellFor>) => {
		const layout = new GroupLayout([], new Set(), count, columns);
		const { shown } = visibleRows(layout.rowCount, cell.height);
		const shownItems = layout.entriesWithin(shown);
		return { rows: shown, shownItems, hiddenItems: count - shownItems };
	};

	it('shows everything while the rows fit', () => {
		expect(capped(100, 6, cellFor(96))).toEqual({ rows: 17, shownItems: 100, hiddenItems: 0 });
	});

	it('caps the rows at the scroll ceiling and counts the items left out', () => {
		const cell = cellFor(96);
		const ceiling = Math.floor(MAX_SCROLL_HEIGHT / cell.height);
		const columns = 10;
		const count = (ceiling + 50) * columns;
		const result = capped(count, columns, cell);
		expect(result.rows).toBe(ceiling);
		expect(result.shownItems).toBe(ceiling * columns);
		expect(result.hiddenItems).toBe(50 * columns);
		expect(result.rows * cell.height).toBeLessThanOrEqual(MAX_SCROLL_HEIGHT);
	});
});

describe('gridMove', () => {
	// 23 items in rows of 5: the last row holds 3 (positions 20, 21, 22).
	const move = (key: string, from: number | null) => gridMove(key, from, 22, 5, 2);

	it('moves by one item sideways and one row up and down', () => {
		expect(move('ArrowRight', 7)).toBe(8);
		expect(move('ArrowLeft', 7)).toBe(6);
		expect(move('ArrowDown', 7)).toBe(12);
		expect(move('ArrowUp', 7)).toBe(2);
	});

	it('stays in the first row going up, keeping the column', () => {
		expect(move('ArrowUp', 3)).toBe(3);
		expect(move('PageUp', 7)).toBe(2);
	});

	it('goes down to the same column of a full last row, to the end of a short one, and stays on the last row', () => {
		expect(gridMove('ArrowDown', 12, 24, 5, 2)).toBe(17);
		expect(move('ArrowDown', 14)).toBe(19);
		expect(move('ArrowDown', 19)).toBe(22);
		expect(move('ArrowDown', 17)).toBe(22);
		expect(move('ArrowDown', 11)).toBe(16);
		expect(move('ArrowDown', 21)).toBe(21);
	});

	it('moves by pages of rows, Home and End', () => {
		expect(gridMove('PageDown', 1, 99, 5, 3)).toBe(16);
		expect(gridMove('PageUp', 16, 99, 5, 3)).toBe(1);
		expect(move('Home', 9)).toBe(0);
		expect(move('End', 9)).toBe(22);
	});

	it('starts at the first item before anything has focus, and ignores other keys', () => {
		expect(move('ArrowDown', null)).toBe(0);
		expect(move('ArrowRight', null)).toBe(0);
		expect(move('a', 3)).toBeNull();
		expect(move('Enter', 3)).toBeNull();
	});
});
