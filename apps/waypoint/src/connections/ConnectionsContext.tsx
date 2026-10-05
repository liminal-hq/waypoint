// The window's connections, supplied by whoever hosts the views: the client and the store that follows Rust
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import type { ConnectionsClient } from './connectionsClient';
import { EMPTY_VIEW, type ConnectionsView } from './connectionsModel';
import { startConnections, type Connections } from './connectionsStore';

const ConnectionsContext = createContext<Connections | null>(null);

interface ConnectionsProviderProps {
	client: ConnectionsClient | undefined;
	children: ReactNode;
}

/** Follows the saved connections and login states for every view below; without a client there are none. */
export function ConnectionsProvider({ client, children }: ConnectionsProviderProps) {
	const [connections, setConnections] = useState<Connections | null>(null);
	useEffect(() => {
		if (!client) return;
		const started = startConnections(client);
		setConnections(started);
		return () => {
			started.stop();
			setConnections(null);
		};
	}, [client]);
	return <ConnectionsContext.Provider value={connections}>{children}</ConnectionsContext.Provider>;
}

/** The window's connections, or `null` where the host has none (a test, a window without remotes). */
export function useConnections(): Connections | null {
	return useContext(ConnectionsContext);
}

const emptyStore = { getState: () => EMPTY_VIEW, subscribe: () => () => {} };

/** Selects from the window's view of the connections; the empty view where there are none. */
export function useConnectionsView<T>(select: (view: ConnectionsView) => T): T {
	const connections = useContext(ConnectionsContext);
	return useStore((connections?.store ?? emptyStore) as Connections['store'], select);
}
