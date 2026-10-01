// Where a dragged tab would land: pure geometry, so the drag can be tested without a layout engine
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The horizontal extent of one tab at the moment a drag began. */
export interface Span {
	left: number;
	right: number;
}

/**
 * The position `from` takes after the drop, given where the dragged tab's centre now is: it lands
 * after every other tab whose own centre it has passed. Always within the strip.
 */
export function dropIndex(spans: readonly Span[], from: number, draggedCentre: number): number {
	let index = 0;
	spans.forEach((span, i) => {
		if (i !== from && (span.left + span.right) / 2 < draggedCentre) index++;
	});
	return index;
}

/**
 * How far the tab at `index` should shift to open a gap for the dragged tab `from` heading to
 * `to`: a tab the drag has passed moves into the space the dragged one left.
 */
export function shiftFor(index: number, from: number, to: number, draggedWidth: number): number {
	if (index === from) return 0;
	if (from < to && index > from && index <= to) return -draggedWidth;
	if (from > to && index < from && index >= to) return draggedWidth;
	return 0;
}
