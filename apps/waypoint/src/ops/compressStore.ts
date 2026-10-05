// Asks what to call a new archive and which format to make: `pickCompression` opens the dialog and resolves to the choice
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ArchiveFormat } from '@liminal-hq/waypoint-protocol/generated/ArchiveFormat';
import { createStore, type StoreApi } from 'zustand/vanilla';

/** What the dialog starts with. */
export interface CompressOptions {
	/** The name the field starts on, without an extension. */
	name: string;
	/** How many items are being packed, for the dialog's description. */
	count: number;
	/** The format that starts selected. */
	format?: ArchiveFormat;
}

/** What the person chose: the name with the extension of the format on it. */
export interface CompressChoice {
	name: string;
	format: ArchiveFormat;
}

export interface CompressRequest {
	options: CompressOptions;
	resolve: (choice: CompressChoice | null) => void;
}

export interface CompressState {
	request: CompressRequest | null;
	/** How many `CompressHost`s are mounted, so a question is not asked where nothing can show it. */
	hosts: number;
}

export type CompressStore = StoreApi<CompressState>;

export function createCompressStore(): CompressStore {
	return createStore<CompressState>()(() => ({ request: null, hosts: 0 }));
}

/** The window's own store, which `CompressHost` follows. */
export const compressStore = createCompressStore();

/** Marks that a dialog is mounted to answer; returns what unmarks it (and cancels one left open). */
export function attachCompressHost(store: CompressStore = compressStore): () => void {
	store.setState((state) => ({ hosts: state.hosts + 1 }));
	return () => {
		store.setState((state) => ({ hosts: state.hosts - 1 }));
		if (store.getState().hosts === 0) store.getState().request?.resolve(null);
	};
}

/**
 * Opens the compress dialog and resolves to the name and format chosen, or `null` when the person
 * cancelled or the window has no dialog. Asking again while one is open answers the first with
 * `null`.
 */
export function pickCompression(
	options: CompressOptions,
	store: CompressStore = compressStore,
): Promise<CompressChoice | null> {
	return new Promise((resolve) => {
		if (store.getState().hosts === 0) {
			resolve(null);
			return;
		}
		store.getState().request?.resolve(null);
		const request: CompressRequest = {
			options,
			resolve: (choice) => {
				if (store.getState().request === request) store.setState({ request: null });
				resolve(choice);
			},
		};
		store.setState({ request });
	});
}
