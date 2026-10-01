// The window's sidebar state: shown or hidden, which sections are collapsed, which folders are expanded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext } from 'react';
import { useStore } from 'zustand';
import { createStore, type StoreApi } from 'zustand/vanilla';

export type SidebarSection = 'places' | 'favourites' | 'workspaces';

/** What the sidebar shows: Places with Favourites, or the Folders tree on its own (SPEC 5.4's Places / Folders switch). */
export type SidebarView = 'places' | 'folders';

export interface SidebarState {
	open: boolean;
	view: SidebarView;
	collapsed: Readonly<Record<SidebarSection, boolean>>;
	/** The `uri` of every folder of the Folders tree the person (or the current folder) opened. */
	expanded: ReadonlySet<string>;
}

export interface SidebarActions {
	toggleOpen(): void;
	setView(view: SidebarView): void;
	toggleSection(section: SidebarSection): void;
	setExpanded(uri: string, expanded: boolean): void;
	/** Expands every folder in `uris` that is not already; changes nothing (and notifies nobody) when all are. */
	expandAll(uris: readonly string[]): void;
}

export type SidebarStore = StoreApi<SidebarState & SidebarActions>;

/**
 * Memory for the window only: nothing is written to `localStorage`, and a new window starts with
 * every section open and the tree closed down to the current folder. Persisting it needs a Rust
 * owner and arrives with settings.
 */
export function createSidebarStore(initial: Partial<SidebarState> = {}): SidebarStore {
	return createStore<SidebarState & SidebarActions>()((set, get) => ({
		open: true,
		view: 'places',
		collapsed: { places: false, favourites: false, workspaces: false },
		expanded: new Set<string>(),
		...initial,
		toggleOpen: () => set((state) => ({ open: !state.open })),
		setView: (view) => set({ view }),
		toggleSection: (section) =>
			set((state) => ({
				collapsed: { ...state.collapsed, [section]: !state.collapsed[section] },
			})),
		setExpanded: (uri, expanded) => {
			if (get().expanded.has(uri) === expanded) return;
			const next = new Set(get().expanded);
			if (expanded) next.add(uri);
			else next.delete(uri);
			set({ expanded: next });
		},
		expandAll: (uris) => {
			const current = get().expanded;
			if (uris.every((uri) => current.has(uri))) return;
			set({ expanded: new Set([...current, ...uris]) });
		},
	}));
}

export const SidebarStoreContext = createContext<SidebarStore | null>(null);

export function useSidebarStore(): SidebarStore {
	const store = useContext(SidebarStoreContext);
	if (!store) throw new Error('useSidebarStore must be used inside a SidebarStoreContext provider');
	return store;
}

export function useSidebarState<T>(select: (state: SidebarState & SidebarActions) => T): T {
	return useStore(useSidebarStore(), select);
}
