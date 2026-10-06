// Sets the window's title everywhere it shows: the title bar's text, the host's title and the page's
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import {
	useOptionalWindowControls,
	useOptionalWindowTitleStore,
} from '../WindowChromeProvider/WindowChromeProvider';

/**
 * Sets the window's title in one call: the text a `<TitleBarTitle fallback="…" />` shows, the host
 * window's own title (`WindowControls.setTitle`, as the app switcher and the task bar show it) and
 * `document.title`. All three get the plain `title`; a `formatTitle` on the provider shapes only
 * the title bar's text.
 *
 * - The latest call wins. A promise keeps the previous title until it resolves, a resolution that a
 *   newer call has overtaken is ignored, and so is a rejection.
 * - `undefined` leaves the title as it was.
 * - Unmounting leaves the last title in place.
 * - Outside a `WindowChromeProvider` nothing happens. A host that cannot set a title, or that throws
 *   or refuses when asked, is skipped.
 */
export function useWindowTitle(title: string | Promise<string> | undefined): void {
	const controls = useOptionalWindowControls();
	const store = useOptionalWindowTitleStore();
	useEffect(() => {
		if (!store || title === undefined) return;
		let current = true;
		const apply = (value: string) => {
			store.set(value);
			document.title = value;
			// A host's `setTitle` may throw as well as reject, so both are caught.
			try {
				void Promise.resolve(controls?.setTitle?.(value)).catch(() => {});
			} catch {
				// The window keeps its last title.
			}
		};
		if (typeof title === 'string') {
			apply(title);
		} else {
			title.then(
				(value) => {
					if (current) apply(value);
				},
				() => {},
			);
		}
		return () => {
			current = false;
		};
	}, [controls, store, title]);
}
