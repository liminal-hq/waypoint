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
import { tf } from '../i18n/messages';
import { useSettings } from '../settings/SettingsContext';
import { useWindowCapabilities, WindowCapabilitiesProvider } from './windowCapabilities';

function ChromeWithControls({
	children,
	formatTitle,
}: {
	children: ReactNode;
	formatTitle?: (title: string) => string;
}) {
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

	return (
		<WindowChromeProvider controls={controls} formatTitle={formatTitle}>
			{children}
		</WindowChromeProvider>
	);
}

/** Wraps a window's content with the chrome state and the window manager's capabilities. */
export function AppWindowChrome({
	children,
	formatTitle,
}: {
	children: ReactNode;
	/** Shapes the title text the title bar shows; the window manager's title stays plain. */
	formatTitle?: (title: string) => string;
}) {
	return (
		<WindowCapabilitiesProvider>
			<ChromeWithControls formatTitle={formatTitle}>{children}</ChromeWithControls>
		</WindowCapabilitiesProvider>
	);
}

/**
 * The Main window's chrome. With the app name setting on, the title bar reads “Waypoint — Trash”
 * and the window manager's title stays “Trash”. Needs the settings above it, and the screen below it,
 * so the title the active tab sets reaches the title bar through the one chrome store.
 */
export function MainWindowChrome({ children }: { children: ReactNode }) {
	const appName = useSettings((value) => value.ui.appNameInTitle);
	const formatTitle = useMemo(
		() => (appName ? (title: string) => tf('window.main.titleWithApp', { title }) : undefined),
		[appName],
	);
	return <AppWindowChrome formatTitle={formatTitle}>{children}</AppWindowChrome>;
}
