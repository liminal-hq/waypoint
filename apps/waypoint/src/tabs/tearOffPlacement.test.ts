// Verifies where a torn-off window is placed from the cursor, the grab point, the scale and the frame margin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { newWindowGeometry } from './tearOffPlacement';

describe('newWindowGeometry', () => {
	it('hangs the visible window from the grab point, in physical pixels', () => {
		// The spike's X11 case: a pointer at logical (1000, 500) is physical (2000, 1000) at 2x.
		expect(
			newWindowGeometry({
				cursor: { x: 2000, y: 1000 },
				scale: 2,
				grab: { x: 120, y: 14 },
				margin: 0,
				inner: { width: 1100, height: 720 },
			}),
		).toEqual({ x: 1760, y: 972, width: 2200, height: 1440, maximised: false });
	});

	it('moves the inner origin out by the frame margin so the visible corner sits at the grab point', () => {
		const flat = newWindowGeometry({
			cursor: { x: 600, y: 400 },
			scale: 1.5,
			grab: { x: 48, y: 20 },
			margin: 0,
			inner: { width: 800, height: 600 },
		});
		const framed = newWindowGeometry({
			cursor: { x: 600, y: 400 },
			scale: 1.5,
			grab: { x: 48, y: 20 },
			margin: 8,
			inner: { width: 800, height: 600 },
		});
		expect(flat).toMatchObject({ x: 528, y: 370, width: 1200, height: 900 });
		expect(framed.x).toBe(flat.x! - 12);
		expect(framed.y).toBe(flat.y! - 12);
	});

	it('allows a position left of or above the origin, which a second monitor can have', () => {
		const geometry = newWindowGeometry({
			cursor: { x: 10, y: 5 },
			scale: 1,
			grab: { x: 48, y: 20 },
			margin: 0,
			inner: { width: 800, height: 600 },
		});
		expect(geometry).toMatchObject({ x: -38, y: -15 });
	});
});
