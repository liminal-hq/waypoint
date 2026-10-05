// The window's copy of the saved connections and of every login's state, brought up to date by Rust's events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ConnectionEntry } from '@liminal-hq/waypoint-protocol/generated/ConnectionEntry';
import type { ConnectionState } from '@liminal-hq/waypoint-protocol/generated/ConnectionState';
import type { ConnectionStatus } from '@liminal-hq/waypoint-protocol/generated/ConnectionStatus';
import type { ConnectionsChanged } from '@liminal-hq/waypoint-protocol/generated/ConnectionsChanged';
import type { ConnectionsOverview } from '@liminal-hq/waypoint-protocol/generated/ConnectionsOverview';
import type { ProtocolsChanged } from '@liminal-hq/waypoint-protocol/generated/ProtocolsChanged';
import type { RecentServer } from '@liminal-hq/waypoint-protocol/generated/RecentServer';

/** What a window knows. Rust is the one writer; this only follows (A81). */
export interface ConnectionsView {
	/** The saved connections' revision; an event at or below it is old news. */
	revision: number;
	connections: readonly ConnectionEntry[];
	recent: readonly RecentServer[];
	/** The manager's revision of the newest state heard. */
	stateRevision: number;
	/** The state of each login by its key (`sftp://me@nas.lan`). A login not here is idle. */
	states: ReadonlyMap<string, ConnectionState>;
	/** True once the first overview arrived. */
	loaded: boolean;
	/**
	 * Which remote protocols are on and which are turned off in Settings → Experimental (D167);
	 * `null` until Rust has said, when nothing is dimmed or hidden.
	 */
	protocols: ProtocolsChanged | null;
}

export const EMPTY_VIEW: ConnectionsView = {
	revision: 0,
	connections: [],
	recent: [],
	stateRevision: 0,
	states: new Map(),
	loaded: false,
	protocols: null,
};

/** The view an overview describes. Events already heard and newer than it are kept. */
export function fromOverview(
	view: ConnectionsView,
	overview: ConnectionsOverview,
): ConnectionsView {
	const saved =
		overview.connections.revision >= view.revision
			? {
					revision: overview.connections.revision,
					connections: overview.connections.connections,
					recent: overview.connections.recent,
				}
			: { revision: view.revision, connections: view.connections, recent: view.recent };
	let states = view.states;
	let stateRevision = view.stateRevision;
	for (const status of overview.statuses) {
		if (status.revision < view.stateRevision && view.states.has(status.key)) continue;
		if (states === view.states) states = new Map(view.states);
		(states as Map<string, ConnectionState>).set(status.key, status.state);
		stateRevision = Math.max(stateRevision, status.revision);
	}
	return { ...saved, states, stateRevision, loaded: true, protocols: view.protocols };
}

/** Applies one change to the saved connections. A change at or below the known revision is dropped. */
export function applyChanged(view: ConnectionsView, change: ConnectionsChanged): ConnectionsView {
	if (change.revision <= view.revision) return view;
	const byId = new Map(view.connections.map((entry) => [entry.connection.id, entry]));
	for (const touched of change.changes) {
		if (touched.entry) byId.set(touched.id, touched.entry);
		else byId.delete(touched.id);
	}
	const order = change.order ?? view.connections.map((entry) => entry.connection.id);
	const connections: ConnectionEntry[] = [];
	for (const id of order) {
		const entry = byId.get(id);
		if (entry) connections.push(entry);
	}
	// A new entry the order did not mention (it cannot happen with Rust's events) goes last.
	for (const [id, entry] of byId) if (!order.includes(id)) connections.push(entry);
	return {
		...view,
		revision: change.revision,
		connections,
		recent: change.recent ?? view.recent,
	};
}

/** Applies one login's new state. A state older than the newest heard for that login is dropped. */
export function applyStatus(view: ConnectionsView, status: ConnectionStatus): ConnectionsView {
	if (status.revision !== 0 && status.revision <= view.stateRevision) return view;
	const states = new Map(view.states);
	states.set(status.key, status.state);
	return { ...view, states, stateRevision: Math.max(view.stateRevision, status.revision) };
}

/** The scheme a login key starts with (`sftp` for `sftp://me@nas.lan`). */
export function schemeOfKey(key: string): string {
	const end = key.indexOf('://');
	return end < 0 ? '' : key.slice(0, end).toLowerCase();
}

/** Whether `scheme` is a protocol the build has that is turned off (D167). Unknown until Rust says. */
export function isProtocolOff(view: ConnectionsView, scheme: string): boolean {
	return view.protocols?.off.includes(scheme.toLowerCase()) ?? false;
}

/**
 * Whether any remote protocol is on, so the Network section is worth showing (D167). It shows
 * until Rust has said which are on, so a window never flashes the section away.
 */
export function anyProtocolOn(view: ConnectionsView): boolean {
	return view.protocols === null || view.protocols.schemes.length > 0;
}

const IDLE: ConnectionState = { kind: 'idle' };

/** The state of a login, idle when nothing is known. */
export function stateOf(view: ConnectionsView, key: string): ConnectionState {
	return view.states.get(key) ?? IDLE;
}

/** The saved connection a login belongs to, when one is saved. */
export function savedFor(view: ConnectionsView, key: string): ConnectionEntry | undefined {
	return view.connections.find((entry) => entry.key === key);
}
