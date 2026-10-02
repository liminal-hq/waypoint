// Reads an element's writing direction and maps the arrow keys onto it
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
