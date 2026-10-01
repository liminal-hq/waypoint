// A one-message live region feed for tab changes made from the menus and the keyboard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSyncExternalStore } from 'react';

let current = '';
let counter = 0;
const listeners = new Set<() => void>();

/** Says `text` through the tab strip's live region. */
export function announce(text: string): void {
	// A repeat of the same text still has to be read out, so a changing zero-width suffix keeps it new.
	counter++;
	current = counter % 2 === 0 ? text : `${text}​`;
	listeners.forEach((listener) => listener());
}

/** Empties the feed, so a message from an earlier window state is not read again. */
export function clearAnnouncement(): void {
	current = '';
	listeners.forEach((listener) => listener());
}

export function useAnnouncement(): string {
	return useSyncExternalStore(
		(listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		() => current,
	);
}
