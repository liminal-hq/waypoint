// The native menu service of a window: the command's client, how icons are drawn for it, and whether it has failed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useMemo, type ReactNode } from 'react';
import { createIconRasteriser, type IconRasteriser } from './menuIconRaster';
import type { NativeMenuClient } from './nativeMenuClient';

/** The platforms whose system menu has been tried: Windows and Linux. */
const SUPPORTED_PLATFORMS = ['linux', 'windows'];

export interface NativeMenuService {
	client: NativeMenuClient;
	rasterise: IconRasteriser;
	/** Set when the command fails, so the window keeps to its own menus until it reloads. */
	state: { failed: boolean };
}

const NativeMenuContext = createContext<NativeMenuService | null>(null);

interface NativeMenuProviderProps {
	client: NativeMenuClient | undefined;
	/** The platform name; the root element's `data-platform` when omitted. */
	platform?: string | undefined;
	/** How icons are drawn; the window's own rasteriser when omitted. */
	rasterise?: IconRasteriser | undefined;
	children: ReactNode;
}

/**
 * Makes `client` the native menu service of the windows below it. Without a client (the demo, a test
 * and any window the command is not for) or on a platform that has not been tried, the menus below
 * stay the page's own.
 */
export function NativeMenuProvider({
	client,
	platform,
	rasterise,
	children,
}: NativeMenuProviderProps) {
	const where = platform ?? document.documentElement.dataset.platform ?? '';
	const supported = SUPPORTED_PLATFORMS.includes(where);
	const service = useMemo<NativeMenuService | null>(
		() =>
			client && supported
				? { client, rasterise: rasterise ?? createIconRasteriser(), state: { failed: false } }
				: null,
		[client, supported, rasterise],
	);
	return <NativeMenuContext.Provider value={service}>{children}</NativeMenuContext.Provider>;
}

/** The window's native menu service, or `null` where there is none to use. */
export function useNativeMenuService(): NativeMenuService | null {
	return useContext(NativeMenuContext);
}
