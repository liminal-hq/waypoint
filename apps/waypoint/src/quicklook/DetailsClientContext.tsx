// The DetailsClient a window uses for previews, supplied by whoever hosts the window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, type ReactNode } from 'react';
import type { DetailsClient } from '../services/detailsClient';

const DetailsClientContext = createContext<DetailsClient | null>(null);

interface DetailsClientProviderProps {
	client: DetailsClient | undefined;
	children: ReactNode;
}

/** Makes `client` the source of text heads and preview addresses below. Without one (the demo, a test) Quick Look is not offered. */
export function DetailsClientProvider({ client, children }: DetailsClientProviderProps) {
	return (
		<DetailsClientContext.Provider value={client ?? null}>{children}</DetailsClientContext.Provider>
	);
}

/** The window's client, or `null` where the host has none. */
export function useDetailsClient(): DetailsClient | null {
	return useContext(DetailsClientContext);
}
