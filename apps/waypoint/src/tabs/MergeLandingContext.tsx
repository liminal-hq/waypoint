// Gives the tab strip the landing line of a drag from another window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext } from 'react';
import { useStore } from 'zustand';
import { createMergeLandingStore, type LandingView, type MergeLandingStore } from './mergeLanding';

const idle = createMergeLandingStore();

export const MergeLandingContext = createContext<MergeLandingStore>(idle);

/** Where tabs dragged here from another window would land, or null while none is over this window. */
export function useLandingView(): LandingView | null {
	return useStore(useContext(MergeLandingContext), (state) => state.view);
}
