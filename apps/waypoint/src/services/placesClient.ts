// The frontend's view of the places service: the fixed Places, the Favourites and the commands that edit them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { Places } from '@liminal-hq/waypoint-protocol/generated/Places';
import type { Unsubscribe } from './vfsClient';

/**
 * What the sidebar needs from the file system plugin's places service. Rust owns the Places and
 * the Favourites (A3): the freedesktop bookmarks file on Linux, so other file managers share them.
 *
 * Every command resolves with the updated `Places`, and rejects with a `VfsError` when it fails.
 * The plugin offers no change event, so the client announces each of its own mutations through
 * `onChange` and every view of the window re-reads from that one signal. Changes another program
 * makes to the bookmarks file are picked up by calling `list()` again (the sidebar does it when
 * the window regains focus).
 */
export interface PlacesClient {
	/** Home, the user folders that exist, and the Favourites in file order. */
	list(): Promise<Places>;
	/** Pins a folder, with an optional label. Pinning a folder that is already pinned changes nothing. */
	addFavourite(location: Location, label?: string): Promise<Places>;
	/** Unpins a folder; a folder that is not pinned is left alone. */
	removeFavourite(location: Location): Promise<Places>;
	/** Labels a Favourite, or clears its label with `null` so the folder's own name shows. */
	renameFavourite(location: Location, label: string | null): Promise<Places>;
	/** Moves a Favourite to position `to` of the Favourites (clamped to the list). */
	moveFavourite(location: Location, to: number): Promise<Places>;
	/** Hears the new `Places` after every mutation made through this client. */
	onChange(listener: (places: Places) => void): Unsubscribe;
}

/** The mutation-announcing half both implementations share. */
export class PlacesChangeEmitter {
	private listeners = new Set<(places: Places) => void>();

	subscribe(listener: (places: Places) => void): Unsubscribe {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	/** Passes `places` on to every listener and returns it, so a command can `return emitter.emit(...)`. */
	emit(places: Places): Places {
		for (const listener of [...this.listeners]) listener(places);
		return places;
	}
}
