// Finds the pane that F5 and Shift+F5 copy and move to: the next pane of the pair the pane is half of
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { ListingSession, SessionState } from '../browse/useListingSession';
import { pairOfTab, paneAfter } from '../tabs/pairLayout';

/**
 * The listing of the pane beside `session`'s: the next pane of the pair (a pair of two has one
 * other; with more, the next in order, wrapping). `null` when `session` is not a pane of a pair,
 * or the other pane has no open listing (it is still opening, or failed to).
 */
export function otherPaneSession(
	snapshot: SessionSnapshot | null,
	session: ListingSession,
	stateFor: (tab: TabId) => SessionState | undefined,
): ListingSession | null {
	if (!snapshot) return null;
	for (const pair of snapshot.pairs) {
		for (const pane of pair.panes) {
			const state = stateFor(pane);
			if (state?.status !== 'ready' || state.session !== session) continue;
			const next = paneAfter(pairOfTab(snapshot.pairs, pane) ?? pair, pane, 1);
			const other = next === undefined ? undefined : stateFor(next);
			return other?.status === 'ready' ? other.session : null;
		}
	}
	return null;
}
