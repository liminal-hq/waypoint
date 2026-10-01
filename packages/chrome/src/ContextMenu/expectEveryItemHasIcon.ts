// Test helper: every selectable row of a context menu, nested submenus included, carries an icon
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expect } from 'vitest';
import type { MenuItem } from './types';

/** Collects the id of every action, checkbox and submenu row (and, recursively, its children) that has no icon. */
export function itemsWithoutIcon(items: readonly MenuItem[]): string[] {
	const missing: string[] = [];
	for (const item of items) {
		if (item.type === 'separator' || item.type === 'section') continue;
		if (item.icon === undefined || item.icon === null) missing.push(item.id);
		if (item.type === 'submenu') missing.push(...itemsWithoutIcon(item.items));
	}
	return missing;
}

/**
 * Fails, naming the ids, when a non-separator item has no `icon`. Every menu item has one so labels
 * line up and the menu reads the same everywhere; a new item cannot ship without its glyph.
 */
export function expectEveryItemHasIcon(items: readonly MenuItem[]): void {
	expect(items.length).toBeGreaterThan(0);
	expect(itemsWithoutIcon(items)).toEqual([]);
}
