// A request to put keyboard focus in a pane once it is on screen and has something to focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';

/** How long a request waits for its pane to become active before it is dropped. */
export const PANE_FOCUS_TIMEOUT_MS = 3000;

let wanted: TabId | null = null;
let expiry: ReturnType<typeof setTimeout> | undefined;
const listeners = new Set<() => void>();

function emit(): void {
	listeners.forEach((listener) => listener());
}

/**
 * F3 and F6 move focus with the active pane; the pane area carries it out when the pane is ready.
 * It may be made before or after the session makes the pane active: the pane area is told either
 * way, and the request waits (up to `PANE_FOCUS_TIMEOUT_MS`) for the pane to be active and listed.
 */
export function requestPaneFocus(tab: TabId): void {
	wanted = tab;
	clearTimeout(expiry);
	expiry = setTimeout(clearPaneFocus, PANE_FOCUS_TIMEOUT_MS);
	emit();
}

/** The tab whose pane should take focus, if one has been asked for. */
export function wantedPaneFocus(): TabId | null {
	return wanted;
}

export function clearPaneFocus(): void {
	clearTimeout(expiry);
	if (wanted === null) return;
	wanted = null;
	emit();
}

/** For `useSyncExternalStore`: renders the pane area again when a request is made or cleared. */
export function subscribePaneFocus(listener: () => void): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}
