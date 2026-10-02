// Reads an element's writing direction, and maps arrow keys and scroll positions onto it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** Whether `element` lays out right to left, from the direction the document set on its root. */
export function isRtl(element: Element | null | undefined): boolean {
	return element ? getComputedStyle(element).direction === 'rtl' : false;
}

/**
 * The key to act on: in a right-to-left layout `ArrowLeft` means "forward" and `ArrowRight`
 * "back", so swapping the two lets a handler written for left to right serve both. Other keys
 * pass through.
 */
export function inlineKey(key: string, rtl: boolean): string {
	if (!rtl) return key;
	if (key === 'ArrowLeft') return 'ArrowRight';
	if (key === 'ArrowRight') return 'ArrowLeft';
	return key;
}

/**
 * Whether there is more to scroll to on the physical left and right. In a right-to-left strip
 * `scrollLeft` runs from `-(scrollWidth - clientWidth)` at the far left to 0 at the start on the
 * right, where a left-to-right strip runs from 0 to the maximum.
 */
export function overflowSides(
	scrollLeft: number,
	scrollWidth: number,
	clientWidth: number,
	rtl: boolean,
): { left: boolean; right: boolean } {
	const max = Math.max(0, scrollWidth - clientWidth);
	const farLeft = rtl ? -max : 0;
	const farRight = rtl ? 0 : max;
	return { left: scrollLeft > farLeft + 1, right: scrollLeft < farRight - 1 };
}

/** The `scrollLeft` that shows the end of the line: the far right when left to right, the far left when right to left. */
export function endScrollLeft(scrollWidth: number, rtl: boolean): number {
	return rtl ? -scrollWidth : scrollWidth;
}

/** How far a vertical wheel turn moves `scrollLeft`: down goes towards the end of the line either way. */
export function wheelScrollDelta(deltaY: number, rtl: boolean): number {
	return rtl ? -deltaY : deltaY;
}
