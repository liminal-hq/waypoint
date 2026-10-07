// The native menu service of a window: the command's client, how icons are drawn for it, and whether it has failed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useEffect, useMemo, type ReactNode } from 'react';
import { createIconStage, type HostedIconStage } from './iconStage';
import {
	createIconRasteriser,
	type IconRasteriser,
	type RasteriserEnvironment,
} from './menuIconRaster';
import type { NativeMenuClient } from './nativeMenuClient';

/** The platforms whose system menu has been tried: Windows and Linux. */
const SUPPORTED_PLATFORMS = ['linux', 'windows'];

export interface NativeMenuService {
	client: NativeMenuClient;
	rasterise: IconRasteriser;
	/** The stage the window's own rasteriser draws in, which `MenuIconStageHost` moves into the tree; none for a rasteriser that was supplied. */
	stage?: HostedIconStage;
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
	/** What the window's own rasteriser draws pixels with; the canvas when omitted (replaced where there is none). */
	environment?: RasteriserEnvironment | undefined;
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
	environment,
	children,
}: NativeMenuProviderProps) {
	const where = platform ?? document.documentElement.dataset.platform ?? '';
	const supported = SUPPORTED_PLATFORMS.includes(where);
	const service = useMemo<NativeMenuService | null>(() => {
		if (!client || !supported) return null;
		if (rasterise) return { client, rasterise, state: { failed: false } };
		const stage = createIconStage();
		return {
			client,
			rasterise: createIconRasteriser(environment, stage),
			stage,
			state: { failed: false },
		};
	}, [client, supported, rasterise, environment]);
	useEffect(() => () => service?.stage?.dispose(), [service]);
	return <NativeMenuContext.Provider value={service}>{children}</NativeMenuContext.Provider>;
}

/** The window's native menu service, or `null` where there is none to use. */
export function useNativeMenuService(): NativeMenuService | null {
	return useContext(NativeMenuContext);
}
