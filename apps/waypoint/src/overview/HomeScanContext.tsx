// The window's measurement of Home, supplied by whoever hosts Overview and the status bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useMemo, type ReactNode } from 'react';
import type { DirScanClient } from '../services/dirScanClient';
import { HomeScanStore } from './homeScanStore';

const HomeScanContext = createContext<HomeScanStore | null>(null);

interface HomeScanProviderProps {
	client: DirScanClient | undefined;
	children: ReactNode;
}

/** Makes one `HomeScanStore` over `client` the window's measurement of Home; without a client Overview does not offer one. */
export function HomeScanProvider({ client, children }: HomeScanProviderProps) {
	const store = useMemo(() => (client ? new HomeScanStore(client) : null), [client]);
	return <HomeScanContext.Provider value={store}>{children}</HomeScanContext.Provider>;
}

/** The window's measurement of Home, or `null` where the host has no directory-size scan. */
export function useHomeScanStore(): HomeScanStore | null {
	return useContext(HomeScanContext);
}
