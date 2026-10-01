// Where a torn-off window opens: the cursor, less where the card was held, in physical pixels
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Geometry } from '@liminal-hq/waypoint-protocol/generated/Geometry';
import type { Point, Size } from '../services/tearoffClient';

/** The ghost card's logical size, which the in-page card draws too. */
export const CARD_SIZE: Size = { width: 240, height: 84 };

/** Where the pointer sits inside the card, so the card hangs from the pointer's top left. */
export const CARD_GRAB: Point = { x: 48, y: 20 };

/**
 * The geometry of a window that opens under the cursor, as the session takes it: the inner
 * (client) origin and the inner size in physical pixels, the size of the window it came from.
 * `cursor` is physical (what the plugin reports), `scale` is that window's scale factor and
 * `margin` its transparent frame margin in logical pixels (the card is held by the visible
 * window's corner, which sits `margin` in from the inner origin). The origin is the plugin's own
 * `cursor - grab * scale` placement, rounded once. A position the monitors cannot hold is dropped
 * by `fit_geometry` on the Rust side, which leaves the window centred.
 */
export function newWindowGeometry(input: {
	cursor: Point;
	scale: number;
	grab: Point;
	margin: number;
	inner: Size;
}): Geometry {
	const { cursor, scale, grab, margin, inner } = input;
	return {
		x: Math.round(cursor.x - (grab.x + margin) * scale),
		y: Math.round(cursor.y - (grab.y + margin) * scale),
		width: Math.max(1, Math.round(inner.width * scale)),
		height: Math.max(1, Math.round(inner.height * scale)),
		maximised: false,
	};
}
