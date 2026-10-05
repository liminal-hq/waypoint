// Whether the Connect dialog or a connection's question is showing in this window, and the one place both are asked to open
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { createStore, type StoreApi } from 'zustand/vanilla';
import type { Answered } from './connectFlow';

/** What the Connect dialog opens on: a new connection (with an address typed or chosen), or a saved one to edit. */
export type ConnectRequest = { mode: 'new'; address?: string } | { mode: 'edit'; id: string };

/** A question waiting for the person, and where the answer goes. */
export interface PendingQuestion {
	error: VfsError;
	resolve(answer: Answered | null): void;
}

export interface ConnectState {
	dialog: ConnectRequest | null;
	question: PendingQuestion | null;
	open(request: ConnectRequest): void;
	close(): void;
	/** Shows the question `error` asks and resolves with the answer, or `null` when cancelled. */
	ask(error: VfsError): Promise<Answered | null>;
	/** Answers the question showing (the question dialogs call it). */
	answer(answer: Answered | null): void;
}

export type ConnectStore = StoreApi<ConnectState>;

export function createConnectStore(): ConnectStore {
	return createStore<ConnectState>((set, get) => ({
		dialog: null,
		question: null,
		open: (request) => set({ dialog: request }),
		close: () => set({ dialog: null }),
		ask: (error) =>
			new Promise<Answered | null>((resolve) => {
				// A question already showing is cancelled: only one is asked at a time.
				get().question?.resolve(null);
				set({ question: { error, resolve } });
			}),
		answer: (answer) => {
			const question = get().question;
			set({ question: null });
			question?.resolve(answer);
		},
	}));
}

/** The window's store. Each window has its own JavaScript heap, so this is one per window. */
export const connectStore: ConnectStore = createConnectStore();

/** Opens the Connect dialog; `ConnectHost` must be mounted for it to show. */
export function openConnectDialog(request: ConnectRequest = { mode: 'new' }): void {
	connectStore.getState().open(request);
}

/** Asks the person the question a connection error holds, through the window's question dialogs. */
export function askQuestion(error: VfsError): Promise<Answered | null> {
	return connectStore.getState().ask(error);
}
