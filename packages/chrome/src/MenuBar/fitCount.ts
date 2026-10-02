// Decides how many menu bar menus fit, so the rest can move into a More menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/**
 * How many of the leading menus to show. `widths` are the buttons' natural widths, `gap` the space
 * between buttons, `available` the width of the bar and `moreWidth` the width of the More button.
 * When everything fits no More button is needed and the result is `widths.length`; otherwise it
 * is the most menus that fit beside More, which may be none. The trailing menus are the ones that
 * collapse.
 */
export function fitCount(
	widths: readonly number[],
	available: number,
	gap: number,
	moreWidth: number,
): number {
	const total = widths.reduce((sum, width, index) => sum + width + (index > 0 ? gap : 0), 0);
	if (total <= available) return widths.length;
	let used = moreWidth;
	let count = 0;
	for (const width of widths) {
		const next = used + gap + width;
		if (next > available) break;
		used = next;
		count++;
	}
	return count;
}
