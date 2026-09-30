// What opening an entry of a listing means: a folder navigates the tab, and more kinds arrive with Open
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { useCallback } from 'react';
import { useVfsClient } from '../browse/VfsClientContext';
import type { Navigation } from './useNavigation';

/** Whether opening `entry` goes into it: a folder, or a link that resolves to one. */
export function isFolder(entry: Entry): boolean {
	return entry.kind === 'directory' || entry.linkTarget === 'directory';
}

/**
 * Enter and double-click. The entry is named by `(handle, id)` and Rust supplies its location, so
 * a folder with a name that is not valid UTF-8 opens as reliably as any other.
 */
export function useOpenEntry(
	navigation: Navigation,
): (entry: Entry, handle: ListingHandle) => void {
	const client = useVfsClient();
	const { goTo } = navigation;
	return useCallback(
		(entry, handle) => {
			if (!isFolder(entry)) return;
			client.entryLocation(handle, entry.id).then(goTo, (error: unknown) => {
				console.warn('could not open the folder', error);
			});
		},
		[client, goTo],
	);
}
