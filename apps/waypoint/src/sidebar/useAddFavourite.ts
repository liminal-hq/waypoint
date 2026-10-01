// Adding a folder to the Favourites from outside the sidebar: the file list's menu and Ctrl+D
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useCallback } from 'react';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { bookmarksSource, workspaceSource } from './favouritesSource';
import { usePlacesClient } from './PlacesClientContext';

/**
 * Pins a folder to whichever list the window's Favourites section is showing: the active
 * workspace's folders, or else the shared bookmarks. Rejects when the edit fails.
 */
export function useAddFavourite(): (location: Location) => Promise<void> {
	const places = usePlacesClient();
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const workspace = snapshot?.workspaces.find((candidate) => candidate.id === snapshot.workspace);
	return useCallback(
		(location) =>
			(workspace ? workspaceSource(api, workspace) : bookmarksSource(places, [])).add(location),
		[api, places, workspace],
	);
}
