// Keeps the sidebar's Places and Favourites current: read at start, after every edit, and on window focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Places } from '@liminal-hq/waypoint-protocol/generated/Places';
import { useEffect, useState } from 'react';
import type { PlacesClient } from '../services/placesClient';

/**
 * The places, or `null` until the first read (and if it fails: the sidebar then shows its
 * sections empty rather than an error, since Home is one click away in the path bar anyway). The
 * plugin has no change event, so a change made by another file manager to the shared bookmarks
 * file is found by reading again when the window regains focus.
 */
export function usePlaces(client: PlacesClient): Places | null {
	const [places, setPlaces] = useState<Places | null>(null);
	useEffect(() => {
		let live = true;
		const read = () =>
			client.list().then(
				(next) => {
					if (live) setPlaces(next);
				},
				(error) => console.warn('could not read the places', error),
			);
		void read();
		const stop = client.onChange((next) => setPlaces(next));
		window.addEventListener('focus', read);
		return () => {
			live = false;
			stop();
			window.removeEventListener('focus', read);
		};
	}, [client]);
	return places;
}
