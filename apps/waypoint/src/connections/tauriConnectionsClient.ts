// The real ConnectionsClient: the file system plugin's connection commands and events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as vfs from '@liminal-hq/waypoint-plugin-vfs';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { Unsubscribe } from '../services/vfsClient';
import type { ConnectionsClient } from './connectionsClient';

/** Adapts an event subscription that resolves later into one that can be stopped at once. */
function follow(start: () => Promise<UnlistenFn>, what: string): Unsubscribe {
	let stopped = false;
	let unlisten: UnlistenFn | undefined;
	start().then(
		(stop) => {
			if (stopped) stop();
			else unlisten = stop;
		},
		(error) => console.error(`could not listen for ${what}`, error),
	);
	return () => {
		stopped = true;
		unlisten?.();
	};
}

/** A `ConnectionsClient` over the `waypoint-vfs` plugin. Create one per window. */
export function createTauriConnectionsClient(): ConnectionsClient {
	return {
		list: () => vfs.listConnections(),
		support: () => vfs.connectionSupport(),
		suggested: () => vfs.suggestedServers(),
		parseAddress: (text) => vfs.parseAddress(text),
		add: (draft) => vfs.addConnection(draft),
		update: (id, draft) => vfs.updateConnection(id, draft),
		duplicate: (id, name) => vfs.duplicateConnection(id, name),
		remove: (id, forgetLogin) => vfs.removeConnection(id, forgetLogin),
		move: (id, to) => vfs.moveConnection(id, to),
		forgetRecent: (key) => vfs.forgetRecentServer(key),
		forgetLogin: (location) => vfs.forgetLogin(location),
		connect: (location, answer = null, remember = false) => vfs.connect(location, answer, remember),
		test: (draft, answer = null, remember = false) => vfs.testConnection(draft, answer, remember),
		disconnect: (location) => vfs.disconnect(location),
		state: (location) => vfs.connectionState(location),
		onChanged: (listener) => follow(() => vfs.onConnectionsChanged(listener), 'connection changes'),
		onState: (listener) => follow(() => vfs.onConnectionState(listener), 'connection states'),
		onProtocols: (listener) => follow(() => vfs.onProtocolsChanged(listener), 'protocol changes'),
	};
}
