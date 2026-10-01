// Where the Favourites section's folders live: the shared bookmarks, or the window's active workspace
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Favourite } from '@liminal-hq/waypoint-protocol/generated/Favourite';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { Workspace } from '@liminal-hq/waypoint-protocol/generated/Workspace';
import type { PlacesClient } from '../services/placesClient';
import type { TabsApi } from '../services/tabsApi';
import { locationLabel } from '../tabs/tabTitle';

/**
 * What `FavouriteList` and the sidebar's editing paths need from whichever list the Favourites
 * section shows. The bookmarks (the freedesktop file other file managers share) and a workspace
 * (a named set Rust keeps in the session store) offer the same four edits, so the rows, the menu
 * and the keyboard paths do not know which one they edit. Each edit resolves when it is applied;
 * the new list arrives through the source's own channel (the places client's change signal, or a
 * session event), which re-renders the section.
 */
export interface FavouritesSource {
	kind: 'bookmarks' | 'workspace';
	favourites: readonly Favourite[];
	/** A workspace keeps folders only, so its rows have no labels of their own to edit. */
	canRename: boolean;
	add(location: Location): Promise<void>;
	remove(location: Location): Promise<void>;
	/** `label` is `null` to clear it. Only offered when `canRename`. */
	rename(location: Location, label: string | null): Promise<void>;
	/** Moves a favourite to position `to` (clamped); resolves to how many there are now. */
	move(location: Location, to: number): Promise<number>;
}

/** The shared bookmarks, edited through the places client. `favourites` is the last read. */
export function bookmarksSource(
	client: PlacesClient,
	favourites: readonly Favourite[],
): FavouritesSource {
	return {
		kind: 'bookmarks',
		favourites,
		canRename: true,
		add: (location) => client.addFavourite(location).then(() => undefined),
		remove: (location) => client.removeFavourite(location).then(() => undefined),
		rename: (location, label) => client.renameFavourite(location, label).then(() => undefined),
		move: (location, to) =>
			client.moveFavourite(location, to).then((next) => next.favourites.length),
	};
}

/** A workspace's folders as favourites, labelled by their own names. */
export function workspaceFavourites(workspace: Workspace): Favourite[] {
	return workspace.locations.map((location) => ({ label: locationLabel(location), location }));
}

/** A workspace's folders, edited as one replaced list through the session. */
export function workspaceSource(api: TabsApi, workspace: Workspace): FavouritesSource {
	const save = (locations: Location[]) => api.setWorkspaceLocations(workspace.id, locations);
	const without = (location: Location) =>
		workspace.locations.filter((candidate) => candidate.uri !== location.uri);
	return {
		kind: 'workspace',
		favourites: workspaceFavourites(workspace),
		canRename: false,
		add: (location) =>
			workspace.locations.some((candidate) => candidate.uri === location.uri)
				? Promise.resolve()
				: save([...workspace.locations, location]),
		remove: (location) => save(without(location)),
		rename: () => Promise.resolve(),
		move: async (location, to) => {
			const rest = without(location);
			// Not one of the workspace's folders: nothing to move.
			if (rest.length === workspace.locations.length) return rest.length;
			const at = Math.max(0, Math.min(to, rest.length));
			rest.splice(at, 0, location);
			await save(rest);
			return rest.length;
		},
	};
}
