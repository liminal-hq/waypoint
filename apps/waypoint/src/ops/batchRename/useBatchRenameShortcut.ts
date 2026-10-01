// The window key for batch rename: Ctrl+F2 opens the dialog for the current selection
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import type { BatchRenameSelection } from './batchRenameApi';
import { openBatchRename } from './batchRenameStore';

/** Whether a key event is Ctrl+F2 (Cmd+F2 where the Meta key stands in for Ctrl). */
export function isBatchRenameKey(event: KeyboardEvent): boolean {
	return (
		event.key === 'F2' &&
		(event.ctrlKey || event.metaKey) &&
		!event.altKey &&
		!event.shiftKey &&
		!event.isComposing
	);
}

/**
 * Opens the batch rename dialog on Ctrl+F2 for whatever `currentSelection` returns, and does
 * nothing when it returns `null` (nothing is selected). The window's file shortcuts (slice 07) and
 * the menu's Rename item call this hook and `openBatchRename` respectively; it has no dependency on
 * the browse stores, so the caller says what "the selection" is.
 */
export function useBatchRenameShortcut(
	currentSelection: () => BatchRenameSelection | null,
	open: (selection: BatchRenameSelection) => void = openBatchRename,
): void {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || !isBatchRenameKey(event)) return;
			const selection = currentSelection();
			if (!selection) return;
			event.preventDefault();
			open(selection);
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [currentSelection, open]);
}
