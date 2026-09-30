// Pure keyboard navigation helpers for menus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from './types';

export function isNavigable(item: MenuItem): boolean {
	return item.type !== 'separator' && item.type !== 'section' && !item.disabled;
}

/** Index of the next navigable item from `from` in `direction`, wrapping; -1 when none exist. */
export function stepIndex(items: MenuItem[], from: number, direction: 1 | -1): number {
	const count = items.length;
	if (count === 0) return -1;
	let index = from;
	for (let i = 0; i < count; i++) {
		index = index < 0 ? (direction === 1 ? 0 : count - 1) : (index + direction + count) % count;
		const item = items[index];
		if (item && isNavigable(item)) return index;
	}
	return -1;
}

export function firstIndex(items: MenuItem[]): number {
	return items.findIndex(isNavigable);
}

export function lastIndex(items: MenuItem[]): number {
	for (let i = items.length - 1; i >= 0; i--) {
		const item = items[i];
		if (item && isNavigable(item)) return i;
	}
	return -1;
}

/**
 * Finds the navigable item whose label starts with `query`. A single-character query starts
 * searching after `from` so repeating a letter cycles through matches; longer queries include it.
 */
export function findByPrefix(items: MenuItem[], query: string, from: number): number {
	const needle = query.toLowerCase();
	const start = needle.length === 1 ? from + 1 : Math.max(from, 0);
	for (let i = 0; i < items.length; i++) {
		const index = (start + i) % items.length;
		const item = items[index];
		if (item && isNavigable(item) && 'label' in item) {
			if (item.label.toLowerCase().startsWith(needle)) return index;
		}
	}
	return -1;
}
