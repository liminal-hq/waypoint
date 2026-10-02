// Pure viewport placement helpers for menus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuPosition } from './types';

export interface Size {
	width: number;
	height: number;
}

export interface Rect {
	left: number;
	top: number;
	right: number;
	bottom: number;
}

export const VIEWPORT_MARGIN = 4;

/** Keeps a box of `size` placed at `position` fully inside the viewport. */
export function clampToViewport(
	position: MenuPosition,
	size: Size,
	viewport: Size,
	margin: number = VIEWPORT_MARGIN,
): MenuPosition {
	const maxX = viewport.width - size.width - margin;
	const maxY = viewport.height - size.height - margin;
	return {
		x: Math.max(margin, Math.min(position.x, maxX)),
		y: Math.max(margin, Math.min(position.y, maxY)),
	};
}

/**
 * Places a submenu beside its parent row: on the right, flipping to the left when there is no room,
 * and the other way round in a right-to-left layout, where a submenu opens towards the inline end.
 */
export function placeSubmenu(
	anchor: Rect,
	size: Size,
	viewport: Size,
	overlap = 4,
	margin: number = VIEWPORT_MARGIN,
	rtl = false,
): MenuPosition {
	const after = anchor.right - overlap;
	const before = anchor.left - size.width + overlap;
	let x = rtl ? before : after;
	if (rtl ? x < margin : x + size.width > viewport.width - margin) {
		x = rtl ? after : before;
	}
	// Align the first submenu row with the parent row (4px accounts for panel padding).
	return clampToViewport({ x, y: anchor.top - 4 }, size, viewport, margin);
}
