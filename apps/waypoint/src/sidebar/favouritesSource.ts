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

/** The edits in flight for each session API, so two quick ones run in turn rather than over each other. */
const queues = new WeakMap<TabsApi, Promise<unknown>>();

/**
 * A workspace's folders, edited as one replaced list through the session. `SetWorkspaceLocations`
 * replaces the whole list, so each edit reads the workspace as it is when the edit runs (not as it
 * was at render) and edits run one after another: two quick edits, such as adding two folders, both
 * land. `workspace` only supplies the rows to draw.
 */
export function workspaceSource(api: TabsApi, workspace: Workspace): FavouritesSource {
	const edit = <T>(change: (current: Location[]) => { next: Location[] | null; result: T }) => {
		const run = async (): Promise<T> => {
			const latest = (await api.getSnapshot()).workspaces.find(
				(candidate) => candidate.id === workspace.id,
			);
			// The workspace was deleted meanwhile: nothing to edit.
			if (!latest) return change([]).result;
			const { next, result } = change([...latest.locations]);
			if (next) await api.setWorkspaceLocations(workspace.id, next);
			return result;
		};
		const turn = (queues.get(api) ?? Promise.resolve()).then(run, run);
		queues.set(
			api,
			turn.catch(() => undefined),
		);
		return turn;
	};
	const without = (current: Location[], location: Location) =>
		current.filter((candidate) => candidate.uri !== location.uri);
	return {
		kind: 'workspace',
		favourites: workspaceFavourites(workspace),
		canRename: false,
		add: (location) =>
			edit((current) => ({
				next: current.some((candidate) => candidate.uri === location.uri)
					? null
					: [...current, location],
				result: undefined,
			})),
		remove: (location) =>
			edit((current) => ({ next: without(current, location), result: undefined })),
		rename: () => Promise.resolve(),
		move: (location, to) =>
			edit((current) => {
				const rest = without(current, location);
				// Not one of the workspace's folders: nothing to move.
				if (rest.length === current.length) return { next: null, result: rest.length };
				const at = Math.max(0, Math.min(to, rest.length));
				rest.splice(at, 0, location);
				return { next: rest, result: rest.length };
			}),
	};
}
