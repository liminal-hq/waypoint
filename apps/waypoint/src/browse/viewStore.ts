// How the window shows listings: list or grid, the grid's icon size, and whether hidden files show
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext } from 'react';
import { useStore } from 'zustand';
import { createStore, type StoreApi } from 'zustand/vanilla';

export type ViewMode = 'list' | 'grid';

/** The grid's icon size range, in pixels (SPEC 5.5). */
export const GRID_SIZE_MIN = 48;
export const GRID_SIZE_MAX = 256;
export const GRID_SIZE_STEP = 8;
export const GRID_SIZE_DEFAULT = 96;

export interface ViewState {
	mode: ViewMode;
	gridSize: number;
	showHidden: boolean;
}

export interface ViewActions {
	setMode(mode: ViewMode): void;
	setGridSize(size: number): void;
	toggleHidden(): void;
}

export type ViewStore = StoreApi<ViewState & ViewActions>;

/** Keeps a size inside the grid's range, on the slider's step. */
export function clampGridSize(size: number): number {
	const stepped = Math.round(size / GRID_SIZE_STEP) * GRID_SIZE_STEP;
	return Math.max(GRID_SIZE_MIN, Math.min(GRID_SIZE_MAX, stepped));
}

/**
 * The window's view choices. They are session state only for now: nothing is written to
 * `localStorage`, and every folder shows the same view. A folder remembering its own view, sort
 * and grouping (SPEC 5.3b) needs a Rust owner and is deferred with the other per-folder settings.
 */
export function createViewStore(initial: Partial<ViewState> = {}): ViewStore {
	return createStore<ViewState & ViewActions>()((set) => ({
		mode: 'list',
		gridSize: GRID_SIZE_DEFAULT,
		showHidden: false,
		...initial,
		setMode: (mode) => set({ mode }),
		setGridSize: (size) => set({ gridSize: clampGridSize(size) }),
		toggleHidden: () => set((state) => ({ showHidden: !state.showHidden })),
	}));
}

export const ViewStoreContext = createContext<ViewStore | null>(null);

export function useViewStore(): ViewStore {
	const store = useContext(ViewStoreContext);
	if (!store) throw new Error('useViewStore must be used inside a ViewStoreContext provider');
	return store;
}

export function useViewState<T>(select: (state: ViewState & ViewActions) => T): T {
	return useStore(useViewStore(), select);
}
