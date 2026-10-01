// The Ctrl+Tab switcher: the most-recently-used walk, held until Ctrl is released
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import { useStore } from 'zustand';
import { createStore } from 'zustand/vanilla';

/** A walk in progress: the tabs in the order Ctrl+Tab visits them, and which one is the candidate. */
export interface SwitcherState {
	order: TabId[];
	index: number;
}

/**
 * The order Ctrl+Tab visits: the active tab, then the rest most recently used first, then any tab
 * the session has not recorded in its MRU list in strip order. Tabs that no longer exist are skipped.
 */
export function switcherOrder(snapshot: SessionSnapshot): TabId[] {
	const open = new Set(snapshot.tabs.map((tab) => tab.id));
	const order: TabId[] = [];
	const add = (id: TabId | null) => {
		if (id !== null && open.has(id) && !order.includes(id)) order.push(id);
	};
	add(snapshot.active);
	snapshot.mru.forEach(add);
	snapshot.tabs.forEach((tab) => add(tab.id));
	return order;
}

const switcherStore = createStore<{ walk: SwitcherState | null }>()(() => ({ walk: null }));

/** Steps the walk by `delta` (1 forward, -1 back), starting it from `snapshot` when none is under way. */
export function stepSwitcher(snapshot: SessionSnapshot, delta: 1 | -1): SwitcherState | null {
	const walk = switcherStore.getState().walk ?? { order: switcherOrder(snapshot), index: 0 };
	if (walk.order.length < 2) return null;
	const index = (walk.index + delta + walk.order.length) % walk.order.length;
	const next = { order: walk.order, index };
	switcherStore.setState({ walk: next });
	return next;
}

export function switcherWalk(): SwitcherState | null {
	return switcherStore.getState().walk;
}

/** Ends the walk and returns the tab it landed on, or `null` when no walk was under way. */
export function endSwitcher(): TabId | null {
	const { walk } = switcherStore.getState();
	switcherStore.setState({ walk: null });
	return walk ? (walk.order[walk.index] ?? null) : null;
}

export function useSwitcherWalk(): SwitcherState | null {
	return useStore(switcherStore, (state) => state.walk);
}
