// The window's chrome context: the Tauri window controls plus the window manager's own menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	getAlwaysOnTop,
	onAlwaysOnTopChanged,
	showSystemWindowMenu,
} from '@liminal-hq/plugin-window-manager';
import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { subscription } from '@liminal-hq/waypoint-chrome/TitleBar/subscription';
import type { WindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/windowControls';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { useMemo, type ReactNode } from 'react';
import { useWindowCapabilities, WindowCapabilitiesProvider } from './windowCapabilities';

function ChromeWithControls({ children }: { children: ReactNode }) {
	const capabilities = useWindowCapabilities();
	const systemWindowMenu = capabilities?.systemWindowMenu === true;

	// The window menu offers "More options…" only where the compositor has a menu to show. Always on
	// Top follows the window manager where it can report it, and otherwise the last request.
	const controls = useMemo<WindowControls>(
		() => ({
			...tauriWindowControls,
			isAlwaysOnTop: async () =>
				(await getAlwaysOnTop()) ?? tauriWindowControls.isAlwaysOnTop?.() ?? false,
			onAlwaysOnTopChange: (listener) => subscription(() => onAlwaysOnTopChanged(listener)),
			...(systemWindowMenu ? { showSystemMenu: showSystemWindowMenu } : {}),
		}),
		[systemWindowMenu],
	);

	return <WindowChromeProvider controls={controls}>{children}</WindowChromeProvider>;
}

/** Wraps a window's content with the chrome state and the window manager's capabilities. */
export function AppWindowChrome({ children }: { children: ReactNode }) {
	return (
		<WindowCapabilitiesProvider>
			<ChromeWithControls>{children}</ChromeWithControls>
		</WindowCapabilitiesProvider>
	);
}
