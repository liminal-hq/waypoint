// The window's tabs as a client-side copy of Rust's session: one snapshot, kept current by events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { createStore, type StoreApi } from 'zustand/vanilla';
import { applyTabsEvent, type TabsApi } from '../services/tabsApi';

export interface TabsState {
	/** `null` until the first snapshot arrives. */
	snapshot: SessionSnapshot | null;
}

export interface TabsHandle {
	store: StoreApi<TabsState>;
	/** Stops following the session. The store keeps its last snapshot. */
	dispose(): void;
}

/**
 * Subscribes to the session first and reads the snapshot second, so no change falls between the
 * two: events that arrive before the snapshot are held and applied on top of it (those already in
 * the snapshot are recognised by their revision and skipped). Rust owns the tabs; nothing here
 * changes them except by applying what Rust says happened.
 */
export function createTabsStore(api: TabsApi): TabsHandle {
	const store = createStore<TabsState>()(() => ({ snapshot: null }));
	const held: SessionEvent[] = [];
	let disposed = false;

	const unsubscribe = api.onEvent((event) => {
		if (disposed) return;
		const { snapshot } = store.getState();
		if (snapshot) store.setState({ snapshot: applyTabsEvent(snapshot, event) });
		else held.push(event);
	});

	void api.getSnapshot().then((initial) => {
		if (disposed) return;
		let snapshot = initial;
		for (const event of held) snapshot = applyTabsEvent(snapshot, event);
		held.length = 0;
		store.setState({ snapshot });
	});

	return {
		store,
		dispose() {
			disposed = true;
			unsubscribe();
		},
	};
}
