// Asks for a folder: `pickDestination` opens the destination dialog and resolves to the one chosen
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { Location } from '../services/opsClient';

/** What a caller asks the destination dialog to say and where it starts. */
export interface DestinationOptions {
	/** The dialog's heading: "Copy 3 items to…". */
	title: string;
	/** What the primary button says: "Copy", "Move", "Choose". */
	confirmLabel: string;
	/** Typed text that is not absolute is read relative to this folder (the pane's, usually). */
	base: Location;
	/** Where the field starts; the most recent destination, then `base`, when omitted. */
	initial?: Location;
	/** The folder the items are in now. */
	origin?: Location;
	/** Refuse `origin` as the answer (a move into the folder the items are already in has nothing to do). */
	forbidOrigin?: boolean;
}

/** A question that is open: its options and the function that answers it. */
export interface DestinationRequest {
	options: DestinationOptions;
	resolve: (destination: Location | null) => void;
}

export interface DestinationState {
	request: DestinationRequest | null;
	/** How many `DestinationHost`s are mounted, so a question is not asked where nothing can show it. */
	hosts: number;
}

export type DestinationStore = StoreApi<DestinationState>;

export function createDestinationStore(): DestinationStore {
	return createStore<DestinationState>()(() => ({ request: null, hosts: 0 }));
}

/** The window's own store, which `DestinationHost` follows. */
export const destinationStore = createDestinationStore();

/** Marks that a dialog is mounted to answer questions; returns what unmarks it (and cancels one left open). */
export function attachHost(store: DestinationStore = destinationStore): () => void {
	store.setState((state) => ({ hosts: state.hosts + 1 }));
	return () => {
		store.setState((state) => ({ hosts: state.hosts - 1 }));
		if (store.getState().hosts === 0) store.getState().request?.resolve(null);
	};
}

/**
 * Opens the destination dialog and resolves to the folder the person chose, or `null` when they
 * cancelled (Esc, Cancel, a click outside) or the window has no dialog to ask in. Asking again
 * while one is open answers the first with `null`. Copy To…, Move To…, F5 without a pair, the
 * conflict dialog's "Choose another location…" and a drop that needs a destination all use it.
 */
export function pickDestination(
	options: DestinationOptions,
	store: DestinationStore = destinationStore,
): Promise<Location | null> {
	return new Promise((resolve) => {
		if (store.getState().hosts === 0) {
			resolve(null);
			return;
		}
		store.getState().request?.resolve(null);
		store.setState({
			request: {
				options,
				resolve: (destination) => {
					// Only the question that is still open is closed by its answer.
					if (store.getState().request?.resolve === answer) store.setState({ request: null });
					resolve(destination);
				},
			},
		});
		const answer = store.getState().request!.resolve;
	});
}
