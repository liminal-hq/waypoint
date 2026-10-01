// A request to put keyboard focus in a pane once it is on screen and has something to focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';

let wanted: TabId | null = null;

/** F3 and F6 move focus with the active pane; the pane area carries it out when the pane is ready. */
export function requestPaneFocus(tab: TabId): void {
	wanted = tab;
}

/** The tab whose pane should take focus, if one has been asked for. */
export function wantedPaneFocus(): TabId | null {
	return wanted;
}

export function clearPaneFocus(): void {
	wanted = null;
}
