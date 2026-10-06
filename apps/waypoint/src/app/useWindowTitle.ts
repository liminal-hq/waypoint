// Keeps the title the window manager shows for this window (the app switcher, the task bar) in step with what the window holds
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useOptionalWindowControls } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { useEffect } from 'react';

/**
 * Sets the window's OS title to `title` and the page's `document.title` with it. The title bar the page
 * draws is separate: windows are undecorated, so this title is seen only outside the window. A host
 * that cannot set it (no window chrome, or controls without `setTitle`) is skipped, and a refusal is not an error worth showing.
 */
export function useWindowTitle(title: string): void {
	const controls = useOptionalWindowControls();
	useEffect(() => {
		document.title = title;
		// A host's `setTitle` may throw as well as reject, so both are caught.
		try {
			void Promise.resolve(controls?.setTitle?.(title)).catch(() => {});
		} catch {
			// The window keeps its last title.
		}
	}, [controls, title]);
}
