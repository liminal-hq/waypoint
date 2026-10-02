// Whether motion is reduced: one answer for the whole app, from the attribute the theme sets
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSyncExternalStore } from 'react';

/**
 * True when the window asks for reduced motion: the `data-motion` attribute the theme engine sets
 * (the OS preference and the Accessibility setting, combined), or before it has run the webview's
 * own media query. Nothing else reads `matchMedia` for this.
 */
export function prefersReducedMotion(root: HTMLElement = document.documentElement): boolean {
	const attribute = root.dataset.motion;
	if (attribute) return attribute === 'reduce';
	try {
		return globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
	} catch {
		return false;
	}
}

function subscribe(onChange: () => void): () => void {
	const observer = new MutationObserver(onChange);
	observer.observe(document.documentElement, {
		attributes: true,
		attributeFilter: ['data-motion'],
	});
	return () => observer.disconnect();
}

/** `prefersReducedMotion()`, re-rendering when the setting or the OS changes it. */
export function useReducedMotion(): boolean {
	return useSyncExternalStore(subscribe, () => prefersReducedMotion());
}
