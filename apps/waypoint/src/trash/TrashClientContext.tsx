// The TrashClient the sidebar and the Trash view talk to, supplied by whoever hosts them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, type ReactNode } from 'react';
import type { TrashClient } from './trashClient';

const TrashClientContext = createContext<TrashClient | null>(null);

interface TrashClientProviderProps {
	client: TrashClient | undefined;
	children: ReactNode;
}

/** Makes `client` the Trash service every view below uses; without one the Trash place shows no count and offers no actions. */
export function TrashClientProvider({ client, children }: TrashClientProviderProps) {
	return (
		<TrashClientContext.Provider value={client ?? null}>{children}</TrashClientContext.Provider>
	);
}

/** The window's Trash client, or `null` where the host has none (the demo, a test). */
export function useTrashClient(): TrashClient | null {
	return useContext(TrashClientContext);
}
