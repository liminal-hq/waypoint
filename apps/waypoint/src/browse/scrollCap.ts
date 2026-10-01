// The webview's scroll-height ceiling and how many rows fit under it (A23)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The tallest scrollable element the webview renders (measured in the milestone 0 spike, A18). */
export const MAX_SCROLL_HEIGHT = 33_554_428;

/** The row height the stylesheet gives when script cannot read one back. */
export const DEFAULT_ROW_HEIGHT = 28;

/**
 * How many rows of `rowHeight` fit under the scroll cap. Computed from the measured height, never a
 * fixed row count: touch mode and larger text make rows taller and the ceiling lower.
 */
export function rowCeiling(rowHeight: number): number {
	return Math.floor(MAX_SCROLL_HEIGHT / rowHeight);
}

/** The rows to lay out and how many of the listing's that leaves out. */
export function visibleRows(count: number, rowHeight: number): { shown: number; hidden: number } {
	const shown = Math.min(count, rowCeiling(rowHeight));
	return { shown, hidden: count - shown };
}

/** Reads the row height the stylesheet set on `element` (`--wp-row-height`, in pixels). */
export function measureRowHeight(element: Element): number {
	const value = getComputedStyle(element).getPropertyValue('--wp-row-height').trim();
	const parsed = Number.parseFloat(value);
	return Number.isFinite(parsed) && parsed > 0 && value.endsWith('px')
		? parsed
		: DEFAULT_ROW_HEIGHT;
}
