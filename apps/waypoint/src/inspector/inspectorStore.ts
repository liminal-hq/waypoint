// The window's Inspector: whether the panel shows, which tab it is on, and how wide it is
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext } from 'react';
import { useStore } from 'zustand';
import { createStore, type StoreApi } from 'zustand/vanilla';

/** The panel's width in pixels: where it starts, and the least and most it may take. */
export const DEFAULT_WIDTH = 320;
export const MIN_WIDTH = 240;
export const MAX_WIDTH = 640;

export type InspectorTab = 'preview' | 'properties';

export interface InspectorState {
	open: boolean;
	/** The tab the panel is on; the toggle reopens it on the last one used. */
	tab: InspectorTab;
	width: number;
}

export interface InspectorActions {
	setOpen(open: boolean): void;
	toggleOpen(): void;
	setTab(tab: InspectorTab): void;
	/** Right-click ▸ Properties: shows the panel on the Properties tab. */
	showProperties(): void;
	setWidth(width: number): void;
}

export type InspectorStore = StoreApi<InspectorState & InspectorActions>;

export function clampWidth(width: number): number {
	return Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, Math.round(width)));
}

/**
 * Memory for the window only, as the Shelf's: whether the panel is open, its tab and its width
 * last as long as the window does.
 */
export function createInspectorStore(initial: Partial<InspectorState> = {}): InspectorStore {
	return createStore<InspectorState & InspectorActions>()((set) => ({
		open: false,
		tab: 'preview',
		width: DEFAULT_WIDTH,
		...initial,
		setOpen: (open) => set({ open }),
		toggleOpen: () => set((state) => ({ open: !state.open })),
		setTab: (tab) => set({ tab }),
		showProperties: () => set({ open: true, tab: 'properties' }),
		setWidth: (width) => set({ width: clampWidth(width) }),
	}));
}

export const InspectorStoreContext = createContext<InspectorStore | null>(null);

export function useInspectorStore(): InspectorStore {
	const store = useContext(InspectorStoreContext);
	if (!store) throw new Error('useInspectorStore must be used inside an InspectorProvider');
	return store;
}

/** The window's store, or `null` outside a provider (a view on its own, with no window around it). */
export function useOptionalInspectorStore(): InspectorStore | null {
	return useContext(InspectorStoreContext);
}

export function useInspectorState<T>(select: (state: InspectorState & InspectorActions) => T): T {
	return useStore(useInspectorStore(), select);
}
