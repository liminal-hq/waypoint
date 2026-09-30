// Default English labels shared by the window chrome components
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** User-visible strings used by the chrome. Apps pass translated values through the `labels` props. */
export interface ChromeLabels {
	restore: string;
	maximise: string;
	minimise: string;
	move: string;
	alwaysOnTop: string;
	/** Entry that opens the compositor's own window menu. */
	systemWindowMenu: string;
	close: string;
	/** Accessible name of the window menu. */
	windowMenu: string;
	/** Accessible name of the group of window buttons. */
	windowControls: string;
}

export const defaultChromeLabels: ChromeLabels = {
	restore: 'Restore',
	maximise: 'Maximise',
	minimise: 'Minimise',
	move: 'Move',
	alwaysOnTop: 'Always on Top',
	systemWindowMenu: 'More window options…',
	close: 'Close',
	windowMenu: 'Window menu',
	windowControls: 'Window controls',
};
