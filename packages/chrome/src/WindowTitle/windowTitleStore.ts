// A tiny external store holding the window's current title, read with `useSyncExternalStore`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export interface WindowTitleStore {
	/** The plain title last set, or `undefined` until something has set one. */
	get: () => string | undefined;
	set: (title: string) => void;
	/** Calls `listener` after each change; returns the function that stops it. */
	subscribe: (listener: () => void) => () => void;
}

/** One store per window; `WindowChromeProvider` creates and owns it. */
export function createWindowTitleStore(): WindowTitleStore {
	let current: string | undefined;
	const listeners = new Set<() => void>();
	return {
		get: () => current,
		set: (title) => {
			if (title === current) return;
			current = title;
			for (const listener of [...listeners]) listener();
		},
		subscribe: (listener) => {
			listeners.add(listener);
			return () => {
				listeners.delete(listener);
			};
		},
	};
}
