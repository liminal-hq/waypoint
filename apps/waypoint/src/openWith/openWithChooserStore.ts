// Whether the application chooser is open, and for which locations: the one place the menu and the command ask it to open
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Handlers } from '@liminal-hq/plugin-mime-apps';
import { createStore, type StoreApi } from 'zustand/vanilla';

/** What the open chooser is choosing an application for. */
export interface ChooserRequest {
	uris: string[];
	handlers: Handlers;
	/** `others` lists the installed applications the menu did not already offer; `all` lists the default and the recommended ones too. */
	scope: 'others' | 'all';
}

export interface OpenWithChooserState {
	request: ChooserRequest | null;
	open(request: ChooserRequest): void;
	close(): void;
}

export type OpenWithChooserStore = StoreApi<OpenWithChooserState>;

/** A store of its own, for a window that wants one and for tests. */
export function createOpenWithChooserStore(): OpenWithChooserStore {
	return createStore<OpenWithChooserState>((set) => ({
		request: null,
		open: (request) => set({ request }),
		close: () => set({ request: null }),
	}));
}

/** The window's store. Each window has its own JavaScript heap, so this is one per window. */
export const openWithChooserStore: OpenWithChooserStore = createOpenWithChooserStore();
