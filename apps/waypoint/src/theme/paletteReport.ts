// What the window's theme decided about the OS palette, for the Appearance page to report
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSyncExternalStore } from 'react';
import type { LiftedColour, PaletteState } from './palette';

/** The theme's answer: whether the system has a palette, whether it is drawn, and what the floor moved. */
export interface PaletteReport {
	/** Whether the system has a usable palette; `null` until the plugin has said. */
	available: boolean | null;
	state: PaletteState;
	lifted: LiftedColour[];
}

const UNKNOWN: PaletteReport = { available: null, state: 'off', lifted: [] };

let current = UNKNOWN;
const listeners = new Set<() => void>();

/** Records the theme's latest decision (one `ThemeRoot` per window writes it). A change in nothing is not announced. */
export function reportPalette(next: PaletteReport): void {
	const same =
		next.available === current.available &&
		next.state === current.state &&
		JSON.stringify(next.lifted) === JSON.stringify(current.lifted);
	if (same) return;
	current = next;
	for (const listener of [...listeners]) listener();
}

/** Forgets the decision, as it is before a theme has answered. */
export function resetPaletteReport(): void {
	reportPalette(UNKNOWN);
}

function subscribe(listener: () => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

/** The theme's latest decision about the palette, re-rendering when it changes. */
export function usePaletteReport(): PaletteReport {
	return useSyncExternalStore(
		subscribe,
		() => current,
		() => UNKNOWN,
	);
}
