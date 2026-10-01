// Gives the file views and the menus the window's clipboard: which rows a cut dims, and whether there is anything to paste
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useMemo } from 'react';
import { createStore } from 'zustand/vanilla';
import { useStore } from 'zustand';
import type { Clipboard } from '../services/opsClient';
import { cutNames } from './clipboardRules';
import type { ClipboardService, ClipboardState } from './clipboardService';

const ClipboardContext = createContext<ClipboardService | null>(null);

export const ClipboardProvider = ClipboardContext.Provider;

/** The window's clipboard; `null` where there is no queue (the views then dim nothing). */
export function useClipboardService(): ClipboardService | null {
	return useContext(ClipboardContext);
}

const NO_CLIPBOARD: Clipboard = { mode: 'copy', items: [], source: 'app', revision: 0 };

// Hooks cannot be skipped, so a window with no clipboard reads an empty one.
const fallback = createStore<ClipboardState>()(() => ({ clipboard: NO_CLIPBOARD }));

/** The clipboard as Rust last said it, following every change from every window. */
export function useClipboard(): Clipboard {
	const service = useClipboardService();
	return useStore(service?.store ?? fallback, (state) => state.clipboard);
}

/** The names a cut dims in the folder `folderUri`, stable while the clipboard and the folder are. */
export function useCutNames(folderUri: string): ReadonlySet<string> {
	const clipboard = useClipboard();
	return useMemo(() => cutNames(clipboard, folderUri), [clipboard, folderUri]);
}
