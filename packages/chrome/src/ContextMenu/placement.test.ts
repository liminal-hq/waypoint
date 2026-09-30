// Tests for viewport placement helpers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { clampToViewport, placeSubmenu } from './placement';

const viewport = { width: 800, height: 600 };
const size = { width: 200, height: 100 };

describe('clampToViewport', () => {
	it('leaves an in-bounds position alone', () => {
		expect(clampToViewport({ x: 50, y: 60 }, size, viewport)).toEqual({ x: 50, y: 60 });
	});

	it('pulls the menu back from the right and bottom edges', () => {
		expect(clampToViewport({ x: 790, y: 590 }, size, viewport)).toEqual({ x: 596, y: 496 });
	});

	it('never goes below the margin, even when larger than the viewport', () => {
		expect(clampToViewport({ x: -20, y: -5 }, size, viewport)).toEqual({ x: 4, y: 4 });
		expect(clampToViewport({ x: 0, y: 0 }, { width: 900, height: 700 }, viewport)).toEqual({
			x: 4,
			y: 4,
		});
	});
});

describe('placeSubmenu', () => {
	it('opens to the right of the parent row', () => {
		const anchor = { left: 100, top: 100, right: 300, bottom: 128 };
		expect(placeSubmenu(anchor, size, viewport)).toEqual({ x: 296, y: 96 });
	});

	it('flips to the left when there is no room on the right', () => {
		const anchor = { left: 600, top: 100, right: 790, bottom: 128 };
		expect(placeSubmenu(anchor, size, viewport).x).toBe(404);
	});
});
