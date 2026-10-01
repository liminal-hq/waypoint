// Verifies the drag geometry for reordering tabs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { clampToZone, dropIndex, shiftFor } from './reorder';

// Four tabs, 100 wide, side by side: centres at 50, 150, 250 and 350.
const spans = [0, 100, 200, 300].map((left) => ({ left, right: left + 100 }));

describe('dropIndex', () => {
	it('stays put until the dragged centre passes a neighbour', () => {
		expect(dropIndex(spans, 1, 150)).toBe(1);
		expect(dropIndex(spans, 1, 200)).toBe(1);
		expect(dropIndex(spans, 1, 260)).toBe(2);
	});

	it('moves to either end and never past them', () => {
		expect(dropIndex(spans, 1, -500)).toBe(0);
		expect(dropIndex(spans, 1, 9000)).toBe(3);
		expect(dropIndex(spans, 0, 9000)).toBe(3);
	});

	it('is the final position, counted without the dragged tab', () => {
		expect(dropIndex(spans, 3, 120)).toBe(1);
	});
});

describe('shiftFor', () => {
	it('opens a gap only for the tabs the drag has passed', () => {
		expect([0, 1, 2, 3].map((i) => shiftFor(i, 0, 2, 100))).toEqual([0, -100, -100, 0]);
		expect([0, 1, 2, 3].map((i) => shiftFor(i, 3, 1, 100))).toEqual([0, 100, 100, 0]);
		expect([0, 1, 2, 3].map((i) => shiftFor(i, 2, 2, 100))).toEqual([0, 0, 0, 0]);
	});
});

describe('clampToZone', () => {
	// Two pinned tabs (0 and 1) and three unpinned (2 to 4).
	it('keeps a pinned tab among the pinned positions', () => {
		expect(clampToZone(4, true, 2, 5)).toBe(1);
		expect(clampToZone(0, true, 2, 5)).toBe(0);
	});

	it('keeps an unpinned tab among the unpinned positions', () => {
		expect(clampToZone(0, false, 2, 5)).toBe(2);
		expect(clampToZone(9, false, 2, 5)).toBe(4);
	});
});
