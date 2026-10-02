// The transparent margin the window frame draws around the content, which is inside the webview but outside the visible window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The transparent margin the window frame draws around the content, in logical pixels (0 where the OS draws the frame). */
export function frameMargin(): number {
	const value = getComputedStyle(document.documentElement).getPropertyValue(
		'--wp-window-shadow-margin',
	);
	const margin = Number.parseFloat(value);
	return Number.isFinite(margin) ? margin : 0;
}

/**
 * Whether `point` (webview coordinates) is outside the visible window: past the edge of the
 * webview, or in the frame's transparent margin, where the pointer already looks to be outside.
 */
export function outsideVisibleWindow(
	point: { x: number; y: number },
	view: { width: number; height: number },
	margin: number,
): boolean {
	return (
		point.x < margin ||
		point.y < margin ||
		point.x >= view.width - margin ||
		point.y >= view.height - margin
	);
}
