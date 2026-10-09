// Whether this window is waiting on the system's administrator prompt, and the one place its Cancel is run
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';

/** A prompt the person has been asked and has not answered. */
export interface PendingPrompt {
	/** Stops the connect that is waiting; the waiting call then ends as cancelled. */
	cancel(): void;
}

export interface ElevationState {
	pending: PendingPrompt | null;
	begin(prompt: PendingPrompt): void;
	end(prompt: PendingPrompt): void;
}

export type ElevationStore = StoreApi<ElevationState>;

export function createElevationStore(): ElevationStore {
	return createStore<ElevationState>((set, get) => ({
		pending: null,
		begin: (prompt) => set({ pending: prompt }),
		// Only the prompt that is showing ends it, so a late answer cannot hide a newer prompt.
		end: (prompt) => {
			if (get().pending === prompt) set({ pending: null });
		},
	}));
}

/** The window's store. Each window has its own JavaScript heap, so this is one per window. */
export const elevationStore: ElevationStore = createElevationStore();
