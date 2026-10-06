// Holds back pictures not yet drawn while a view scrolls, so a fast scroll does not load and decode each one it passes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext } from 'react';

/** Whether a view is scrolling, for the thumbnails in it to read. */
export interface ThumbnailPause {
	paused(): boolean;
	subscribe(listener: () => void): () => void;
	set(paused: boolean): void;
}

/** A pause for one view; it starts unpaused. */
export function createThumbnailPause(): ThumbnailPause {
	let paused = false;
	const listeners = new Set<() => void>();
	return {
		paused: () => paused,
		subscribe(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		set(next) {
			if (next === paused) return;
			paused = next;
			for (const listener of [...listeners]) listener();
		},
	};
}

/**
 * The pause of the view a `Thumbnail` is in, or `null` where pictures are never held back (the list,
 * the Shelf). Under a pause, a picture already drawn in its frame stays; one not drawn yet waits,
 * with the icon in its place, until the pause ends.
 */
export const ThumbnailPauseContext = createContext<ThumbnailPause | null>(null);
