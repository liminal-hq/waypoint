// Verifies the sidebar's window state: shown or hidden, collapsed sections, expanded folders
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createSidebarStore } from './sidebarStore';

describe('the sidebar store', () => {
	it('starts open, every section open, nothing expanded', () => {
		const state = createSidebarStore().getState();
		expect(state.open).toBe(true);
		expect(state.collapsed).toEqual({ places: false, favourites: false });
		expect(state.expanded.size).toBe(0);
	});

	it('toggles the panel and each section on its own', () => {
		const store = createSidebarStore();
		store.getState().toggleOpen();
		store.getState().toggleSection('favourites');
		expect(store.getState().open).toBe(false);
		expect(store.getState().collapsed).toEqual({ places: false, favourites: true });
		store.getState().toggleSection('favourites');
		expect(store.getState().collapsed.favourites).toBe(false);
	});

	it('expands and collapses folders, and notifies only when something changed', () => {
		const store = createSidebarStore();
		const listener = vi.fn();
		store.subscribe(listener);
		store.getState().setExpanded('a', true);
		store.getState().setExpanded('a', true);
		store.getState().expandAll(['a']);
		expect(listener).toHaveBeenCalledTimes(1);
		store.getState().expandAll(['a', 'b', 'c']);
		expect([...store.getState().expanded]).toEqual(['a', 'b', 'c']);
		store.getState().setExpanded('b', false);
		expect([...store.getState().expanded]).toEqual(['a', 'c']);
		expect(listener).toHaveBeenCalledTimes(3);
	});
});
