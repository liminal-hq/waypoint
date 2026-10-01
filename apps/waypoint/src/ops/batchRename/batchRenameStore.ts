// Whether the batch rename dialog is open, and for what: the one place the menus and the shortcut ask it to open
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { BatchRenameSelection } from './batchRenameApi';

export interface BatchRenameState {
	/** What the open dialog is renaming; `null` while it is closed. */
	selection: BatchRenameSelection | null;
	open(selection: BatchRenameSelection): void;
	close(): void;
}

export type BatchRenameStore = StoreApi<BatchRenameState>;

/** A store of its own, for a window that wants one and for tests. */
export function createBatchRenameStore(): BatchRenameStore {
	return createStore<BatchRenameState>((set) => ({
		selection: null,
		open: (selection) => set({ selection }),
		close: () => set({ selection: null }),
	}));
}

/** The window's store. Each window has its own JavaScript heap, so this is one per window. */
export const batchRenameStore: BatchRenameStore = createBatchRenameStore();

/**
 * Opens the batch rename dialog for a selection. The context menus and the Rename action call this
 * (when more than one entry is selected); `BatchRenameHost` must be mounted for it to show.
 */
export function openBatchRename(selection: BatchRenameSelection): void {
	batchRenameStore.getState().open(selection);
}
