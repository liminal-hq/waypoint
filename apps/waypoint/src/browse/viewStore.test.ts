// Verifies the window's view choices: defaults, the grid size range and that nothing is persisted
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import {
	clampGridSize,
	createViewStore,
	GRID_SIZE_DEFAULT,
	GRID_SIZE_MAX,
	GRID_SIZE_MIN,
} from './viewStore';

describe('the view store', () => {
	it('starts as a list of default-sized icons with hidden files hidden', () => {
		expect(createViewStore().getState()).toMatchObject({
			mode: 'list',
			gridSize: GRID_SIZE_DEFAULT,
			showHidden: false,
		});
	});

	it('switches mode and toggles hidden files', () => {
		const store = createViewStore();
		store.getState().setMode('grid');
		store.getState().toggleHidden();
		expect(store.getState()).toMatchObject({ mode: 'grid', showHidden: true });
		store.getState().toggleHidden();
		expect(store.getState().showHidden).toBe(false);
	});

	it('keeps the icon size inside 48 to 256 px, on the slider step', () => {
		expect(clampGridSize(10)).toBe(GRID_SIZE_MIN);
		expect(clampGridSize(1000)).toBe(GRID_SIZE_MAX);
		expect(clampGridSize(101)).toBe(104);
		const store = createViewStore();
		store.getState().setGridSize(999);
		expect(store.getState().gridSize).toBe(GRID_SIZE_MAX);
	});

	it('writes nothing to localStorage', () => {
		const set = vi.spyOn(Storage.prototype, 'setItem');
		const store = createViewStore();
		store.getState().setMode('grid');
		store.getState().setGridSize(200);
		expect(set).not.toHaveBeenCalled();
		set.mockRestore();
	});
});
