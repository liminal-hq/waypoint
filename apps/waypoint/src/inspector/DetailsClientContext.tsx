// The DetailsClient a window uses, supplied by whoever hosts the window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, type ReactNode } from 'react';
import type { DetailsClient } from '../services/detailsClient';

const DetailsClientContext = createContext<DetailsClient | null>(null);

/**
 * Makes `client` the service the Inspector reads entry details, folder sizes and previews from.
 * Without one (the demo, a test) the Inspector shows only what the listing already knows.
 */
export function DetailsClientProvider({
	client,
	children,
}: {
	client: DetailsClient | undefined;
	children: ReactNode;
}) {
	return (
		<DetailsClientContext.Provider value={client ?? null}>{children}</DetailsClientContext.Provider>
	);
}

/** The window's client, or `null` where the host has none. */
export function useDetailsClient(): DetailsClient | null {
	return useContext(DetailsClientContext);
}
