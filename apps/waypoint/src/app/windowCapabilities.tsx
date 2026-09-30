// Shares what the window manager can do (always on top, its own window menu) with the window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getCapabilities, type WindowCapabilities } from '@liminal-hq/plugin-window-manager';
import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';

const WindowCapabilitiesContext = createContext<WindowCapabilities | null>(null);

/**
 * Reads the window manager's capabilities once and shares them. They stay `null` until known
 * and for good outside Tauri, so features that depend on them stay hidden rather than
 * appearing and then disappearing.
 */
export function WindowCapabilitiesProvider({ children }: { children: ReactNode }) {
	const [capabilities, setCapabilities] = useState<WindowCapabilities | null>(null);

	useEffect(() => {
		let active = true;
		getCapabilities()
			.then((value) => {
				if (active) setCapabilities(value);
			})
			.catch(() => {
				// Not in Tauri, or the plugin is unavailable.
			});
		return () => {
			active = false;
		};
	}, []);

	return (
		<WindowCapabilitiesContext.Provider value={capabilities}>
			{children}
		</WindowCapabilitiesContext.Provider>
	);
}

export function useWindowCapabilities(): WindowCapabilities | null {
	return useContext(WindowCapabilitiesContext);
}
