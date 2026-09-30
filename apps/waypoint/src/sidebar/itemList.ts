// Arrow-key focus movement between the items of a Places or Favourites list
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { KeyboardEvent } from 'react';

/** The attribute that marks an element as one of a list's items. */
export const ITEM_ATTRIBUTE = 'data-sidebar-item';

/**
 * Up, Down, Home and End move focus between the list's items (each stays a tab stop as well, so a
 * short list never needs arrows). Returns whether the key was one of them.
 */
export function moveFocusInList(event: KeyboardEvent<HTMLElement>): boolean {
	if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return false;
	if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(event.key)) return false;
	const items = [...event.currentTarget.querySelectorAll<HTMLElement>(`[${ITEM_ATTRIBUTE}]`)];
	const from = items.indexOf(
		(event.target as HTMLElement).closest<HTMLElement>(`[${ITEM_ATTRIBUTE}]`)!,
	);
	if (from < 0) return false;
	const to =
		event.key === 'Home'
			? 0
			: event.key === 'End'
				? items.length - 1
				: Math.max(0, Math.min(items.length - 1, from + (event.key === 'ArrowDown' ? 1 : -1)));
	event.preventDefault();
	items[to]?.focus();
	return true;
}
