// Whether Quick Look is open, and for which listing and position: the one place a view asks it to open
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { OpenHandler } from '../browse/useListInteractions';
import type { ListingSession } from '../browse/useListingSession';

/** How a view moves its focus for a key, so the overlay steps the way the view does. */
export type ViewMove = (key: string, from: number | null, last: number) => number | null;

/** What is previewed is the focused entry of `session`, so a patch that moves it moves the preview with it. */
export interface QuickLookRequest {
	session: ListingSession;
	/** The view's navigation, for Up and Down in a grid. */
	move: ViewMove;
	/** How the view opens an entry (its Enter and double-click), for the Open button. */
	onOpen: OpenHandler | undefined;
}

export interface QuickLookState {
	request: QuickLookRequest | null;
	/** How many hosts are mounted; a view asks only where something will show it. */
	hosts: number;
	open(request: QuickLookRequest): boolean;
	close(): void;
	attach(): () => void;
}

export type QuickLookStore = StoreApi<QuickLookState>;

/** A store of its own, for a window that wants one and for tests. */
export function createQuickLookStore(): QuickLookStore {
	return createStore<QuickLookState>((set, get) => ({
		request: null,
		hosts: 0,
		open(request) {
			if (get().hosts === 0) return false;
			set({ request });
			return true;
		},
		close: () => set({ request: null }),
		attach() {
			set((state) => ({ hosts: state.hosts + 1 }));
			return () => set((state) => ({ hosts: state.hosts - 1, request: null }));
		},
	}));
}

/** The window's store. Each window has its own JavaScript heap, so this is one per window. */
export const quickLookStore: QuickLookStore = createQuickLookStore();
