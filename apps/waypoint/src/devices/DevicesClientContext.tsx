// The DevicesClient the sidebar talks to, supplied by whoever hosts it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, type ReactNode } from 'react';
import type { DevicesClient } from './devicesClient';

const DevicesClientContext = createContext<DevicesClient | null>(null);

interface DevicesClientProviderProps {
	client: DevicesClient | undefined;
	children: ReactNode;
}

/** Makes `client` the volumes service every view below uses; without one the sidebar has no Devices section. */
export function DevicesClientProvider({ client, children }: DevicesClientProviderProps) {
	return (
		<DevicesClientContext.Provider value={client ?? null}>{children}</DevicesClientContext.Provider>
	);
}

/** The window's Devices client, or `null` where the host has none (a test). */
export function useDevicesClient(): DevicesClient | null {
	return useContext(DevicesClientContext);
}
