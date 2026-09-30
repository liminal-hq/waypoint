// Detects pointer targets that belong to controls rather than empty title bar space
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const INTERACTIVE_SELECTOR = [
	'button',
	'a[href]',
	'input',
	'select',
	'textarea',
	'summary',
	'[role="button"]',
	'[role="menu"]',
	'[role="menuitem"]',
	'[contenteditable=""]',
	'[contenteditable="true"]',
	'[data-window-menu-exclude]',
	'[data-wp-controls]',
].join(',');

/** True when the event target sits inside an interactive element or an explicitly excluded region. */
export function isInteractiveTarget(target: EventTarget | null): boolean {
	return target instanceof Element && target.closest(INTERACTIVE_SELECTOR) !== null;
}
