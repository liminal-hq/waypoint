// Button layout and titlebar action model shared with the host's appearance settings
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** Window-manager button tokens. The chrome renders the ones it supports and skips the rest. */
export type ChromeButton =
	| 'appMenu'
	| 'windowMenu'
	| 'minimise'
	| 'maximise'
	| 'close'
	| 'keepAbove'
	| 'keepBelow'
	| 'shade'
	| 'stick'
	| 'help';

/** Which buttons sit on each side of the title bar, in visual order from left to right. */
export interface ButtonLayout {
	start: ChromeButton[];
	end: ChromeButton[];
}

export type TitlebarAction =
	'toggleMaximise' | 'toggleShade' | 'minimise' | 'lower' | 'menu' | 'none';

/** What clicking empty title bar space does. */
export interface TitlebarActions {
	doubleClick: TitlebarAction;
	middleClick: TitlebarAction;
	rightClick: TitlebarAction;
}

export const DEFAULT_BUTTON_LAYOUT: ButtonLayout = {
	start: [],
	end: ['minimise', 'maximise', 'close'],
};

export const DEFAULT_TITLEBAR_ACTIONS: TitlebarActions = {
	doubleClick: 'toggleMaximise',
	middleClick: 'none',
	rightClick: 'menu',
};
