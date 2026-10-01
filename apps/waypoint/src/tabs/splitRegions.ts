// The four split regions of the file area that a dragged tab can be dropped on, and which one a point is in
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Point } from '../dnd/dragSession';
import type { Edge, Rect } from './dragLayout';
import { SPLIT_ZONE_SLACK_PX } from './dragTiming';

/** The edges in the order the overlay draws them. */
export const SPLIT_EDGES: readonly Edge[] = ['left', 'right', 'top', 'bottom'];

/** The share of the area's width the left and right regions each take; the centre column has the rest. */
export const SIDE_FRACTION = 1 / 3;

/**
 * Where the pointer answers to `edge`: the left and right thirds at full height, and the centre
 * column split into its upper and lower halves. Together they tile the area.
 */
export function zoneRect(area: Rect, edge: Edge): Rect {
	const width = area.right - area.left;
	const side = width * SIDE_FRACTION;
	const middle = (area.top + area.bottom) / 2;
	switch (edge) {
		case 'left':
			return { left: area.left, top: area.top, right: area.left + side, bottom: area.bottom };
		case 'right':
			return { left: area.right - side, top: area.top, right: area.right, bottom: area.bottom };
		case 'top':
			return { left: area.left + side, top: area.top, right: area.right - side, bottom: middle };
		case 'bottom':
			return { left: area.left + side, top: middle, right: area.right - side, bottom: area.bottom };
	}
}

/** The half of the area the new pane would take, which is what the overlay tints. */
export function previewRect(area: Rect, edge: Edge): Rect {
	const centre = { x: (area.left + area.right) / 2, y: (area.top + area.bottom) / 2 };
	switch (edge) {
		case 'left':
			return { ...area, right: centre.x };
		case 'right':
			return { ...area, left: centre.x };
		case 'top':
			return { ...area, bottom: centre.y };
		case 'bottom':
			return { ...area, top: centre.y };
	}
}

function within(rect: Rect, point: Point, slack = 0): boolean {
	return (
		point.x >= rect.left - slack &&
		point.x <= rect.right + slack &&
		point.y >= rect.top - slack &&
		point.y <= rect.bottom + slack
	);
}

/**
 * The region the pointer is in, or null outside the file area. `previous` is the region it was in
 * a moment ago: it is kept while the pointer is within `slack` of it, so a pointer riding a border
 * (or the area's edge, where the strip's reorder is the other choice) does not flicker.
 */
export function zoneAt(
	area: Rect,
	point: Point,
	previous: Edge | null = null,
	slack: number = SPLIT_ZONE_SLACK_PX,
): Edge | null {
	if (area.right - area.left <= 0 || area.bottom - area.top <= 0) return null;
	if (previous && within(area, point, slack) && within(zoneRect(area, previous), point, slack)) {
		return previous;
	}
	if (!within(area, point)) return null;
	return SPLIT_EDGES.find((edge) => within(zoneRect(area, edge), point)) ?? null;
}
