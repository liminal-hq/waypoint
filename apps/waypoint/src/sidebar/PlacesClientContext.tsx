// The PlacesClient the sidebar talks to, supplied by whoever hosts it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, type ReactNode } from 'react';
import type { PlacesClient } from '../services/placesClient';

const PlacesClientContext = createContext<PlacesClient | null>(null);

interface PlacesClientProviderProps {
	client: PlacesClient;
	children: ReactNode;
}

/** Makes `client` the places service every sidebar view below reads from and edits through. */
export function PlacesClientProvider({ client, children }: PlacesClientProviderProps) {
	return <PlacesClientContext.Provider value={client}>{children}</PlacesClientContext.Provider>;
}

export function usePlacesClient(): PlacesClient {
	const client = useContext(PlacesClientContext);
	if (!client) throw new Error('usePlacesClient must be used inside a PlacesClientProvider');
	return client;
}
