// Keeps one window's ConnectionsView current: the first overview, then every change and state Rust sends
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { ConnectionsClient } from './connectionsClient';
import {
	applyChanged,
	applyStatus,
	EMPTY_VIEW,
	fromOverview,
	type ConnectionsView,
} from './connectionsModel';

export type ConnectionsStore = StoreApi<ConnectionsView>;

/** The store and the client it follows; `stop` ends the subscriptions. */
export interface Connections {
	client: ConnectionsClient;
	store: ConnectionsStore;
	/** Reads the overview again (after a window was asleep, or to recover from a missed event). */
	reload(): Promise<void>;
	stop(): void;
}

/**
 * Starts following Rust: subscribes first, then reads the overview, so nothing between the two is
 * missed (an event newer than the overview wins, an older one is dropped by its revision).
 */
export function startConnections(client: ConnectionsClient): Connections {
	const store = createStore<ConnectionsView>(() => EMPTY_VIEW);
	let live = true;
	const stopChanged = client.onChanged((change) => {
		if (live) store.setState((view) => applyChanged(view, change), true);
	});
	const stopState = client.onState((status) => {
		if (live) store.setState((view) => applyStatus(view, status), true);
	});
	const reload = () =>
		client.list().then(
			(overview) => {
				if (live) store.setState((view) => fromOverview(view, overview), true);
			},
			(error: unknown) => console.warn('could not read the saved connections', error),
		);
	void reload();
	return {
		client,
		store,
		reload,
		stop() {
			live = false;
			stopChanged();
			stopState();
		},
	};
}
