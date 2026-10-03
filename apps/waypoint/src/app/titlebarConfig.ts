// Chooses the title bar's button style, layout, actions and title alignment
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TitlebarPreferences } from '@liminal-hq/plugin-system-appearance';
import {
	DEFAULT_BUTTON_LAYOUT,
	DEFAULT_TITLEBAR_ACTIONS,
	type ButtonLayout,
	type TitlebarActions,
} from '@liminal-hq/waypoint-chrome/TitleBar/buttonLayout';
import type { ControlsStyle } from '@liminal-hq/waypoint-chrome/TitleBar/windowControls';

export interface TitlebarConfig {
	controlsStyle: ControlsStyle;
	buttonLayout: ButtonLayout;
	titlebarActions: TitlebarActions;
	titleAlign: 'center' | 'start';
}

const STYLE_BY_DESKTOP: Record<TitlebarPreferences['desktopEnvironment'], ControlsStyle> = {
	gnome: 'gnome',
	kde: 'kde',
	// Cinnamon draws the same window controls as GNOME (the compact square `cinnamon` style stays in
	// the chrome for a later choice). Which buttons appear and where still follow Cinnamon's settings.
	cinnamon: 'gnome',
	windows: 'win11',
	// No dedicated style yet: these fall back to the GNOME look.
	mate: 'gnome',
	xfce: 'gnome',
	macos: 'gnome',
	unknown: 'gnome',
};

/**
 * The title bar configuration for the OS's own preferences. With no reading (before the first
 * one arrives, or outside Tauri) it falls back to the platform default: Windows gets caption
 * buttons and a left-aligned title, everything else the GNOME style, a centred title, and the
 * buttons on the right. The visual style follows the desktop environment; which buttons appear,
 * where, and what the title bar's clicks do follow the user's own settings.
 */
export function titlebarConfigFor(
	preferences: TitlebarPreferences | null,
	platform: string | undefined,
): TitlebarConfig {
	if (preferences === null) {
		const windows = platform === 'windows';
		return {
			controlsStyle: windows ? 'win11' : 'gnome',
			buttonLayout: DEFAULT_BUTTON_LAYOUT,
			titlebarActions: DEFAULT_TITLEBAR_ACTIONS,
			titleAlign: windows ? 'start' : 'center',
		};
	}
	return {
		controlsStyle: STYLE_BY_DESKTOP[preferences.desktopEnvironment],
		buttonLayout: preferences.buttonLayout,
		titlebarActions: preferences.actions,
		titleAlign: preferences.desktopEnvironment === 'windows' ? 'start' : 'center',
	};
}
