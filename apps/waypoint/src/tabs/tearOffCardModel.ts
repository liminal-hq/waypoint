// What the in-page mini window card shows: nothing, or the payload while the pointer is over this window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { GhostPayload } from '../services/tearoffClient';

export interface TearCardState {
	/** The card to draw, or null while none should show. */
	payload: GhostPayload | null;
}

export type TearCardStore = StoreApi<TearCardState>;

export function createTearCardStore(): TearCardStore {
	return createStore<TearCardState>(() => ({ payload: null }));
}
