// The PropertiesWindowClient a main window uses to open Properties windows, supplied by whoever hosts the window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, type ReactNode } from 'react';
import type { PropertiesWindowClient } from '../services/propertiesWindowClient';

const PropertiesWindowContext = createContext<PropertiesWindowClient | null>(null);

/**
 * Makes `client` the service that opens Properties windows. Without one (the demo, a test) the
 * window has no Properties window to offer: Alt+Enter, the menu item and the Inspector's button
 * are hidden.
 */
export function PropertiesWindowProvider({
	client,
	children,
}: {
	client: PropertiesWindowClient | undefined;
	children: ReactNode;
}) {
	return (
		<PropertiesWindowContext.Provider value={client ?? null}>
			{children}
		</PropertiesWindowContext.Provider>
	);
}

/** The window's client, or `null` where the host has none. */
export function usePropertiesWindowClient(): PropertiesWindowClient | null {
	return useContext(PropertiesWindowContext);
}
