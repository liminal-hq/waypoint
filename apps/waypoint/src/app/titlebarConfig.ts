// Chooses the title bar's button style, layout, actions and title alignment for a platform
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

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

/**
 * The layout used before, or without, a reading of the OS's own preferences: Windows gets
 * caption-style buttons and a left-aligned title, everything else the GNOME style with a
 * centred title. The buttons sit on the right on every platform Waypoint targets.
 */
export function fallbackTitlebarConfig(platform: string | undefined): TitlebarConfig {
	const windows = platform === 'windows';
	return {
		controlsStyle: windows ? 'win11' : 'gnome',
		buttonLayout: DEFAULT_BUTTON_LAYOUT,
		titlebarActions: DEFAULT_TITLEBAR_ACTIONS,
		titleAlign: windows ? 'start' : 'center',
	};
}
