// Reference-counted page scroll lock that keeps the layout from shifting
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

let locks = 0;
let saved: { overflow: string; paddingInlineEnd: string } | null = null;

/** Locks page scrolling; the scrollbar's width is added as padding so the page does not jump. */
export function lockScroll(): void {
	locks += 1;
	if (locks > 1) return;
	const root = document.documentElement;
	saved = { overflow: root.style.overflow, paddingInlineEnd: root.style.paddingInlineEnd };
	const scrollbar = Math.max(0, window.innerWidth - root.clientWidth);
	if (scrollbar > 0) {
		const current = Number.parseFloat(getComputedStyle(root).paddingInlineEnd) || 0;
		root.style.paddingInlineEnd = `${current + scrollbar}px`;
	}
	root.style.overflow = 'hidden';
}

/** Releases one lock; the page scrolls again once the last one is gone. */
export function unlockScroll(): void {
	if (locks === 0) return;
	locks -= 1;
	if (locks > 0 || !saved) return;
	const root = document.documentElement;
	root.style.overflow = saved.overflow;
	root.style.paddingInlineEnd = saved.paddingInlineEnd;
	saved = null;
}
