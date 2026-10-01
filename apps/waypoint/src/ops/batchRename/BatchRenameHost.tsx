// Shows the batch rename dialog when something asks for it: mount one in each window that can rename
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useStore } from 'zustand';
import type { BatchRenameApi } from './batchRenameApi';
import { batchRenameStore, type BatchRenameStore } from './batchRenameStore';
import { BatchRenameDialog } from './BatchRenameDialog';

export interface BatchRenameHostProps {
	api: BatchRenameApi;
	/** Tells the window what happened after the dialog has closed (a notice or a live region). */
	announce?: (message: string) => void;
	/** The store the dialog follows; the window's own by default. */
	store?: BatchRenameStore;
	debounceMs?: number;
}

export function BatchRenameHost({
	api,
	announce,
	store = batchRenameStore,
	debounceMs,
}: BatchRenameHostProps) {
	const selection = useStore(store, (state) => state.selection);
	if (!selection) return null;
	return (
		<BatchRenameDialog
			open
			selection={selection}
			api={api}
			announce={announce}
			debounceMs={debounceMs}
			onClose={() => store.getState().close()}
		/>
	);
}
