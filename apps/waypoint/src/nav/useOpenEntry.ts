// What opening an entry of a listing means: a folder navigates the tab or opens a tab
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useMemo } from 'react';
import { useVfsClient } from '../browse/VfsClientContext';
import { useTabActions } from '../tabs/tabActions';
import type { Navigation } from './useNavigation';

/** Whether opening `entry` goes into it: a folder, or a link that resolves to one. */
export function isFolder(entry: Entry): boolean {
	return entry.kind === 'directory' || entry.linkTarget === 'directory';
}

export interface EntryOpeners {
	/** Enter and double-click. */
	open: (entry: Entry, handle: ListingHandle) => void;
	/** Middle-click: a folder opens in a tab beside this one, and anything else does nothing. */
	openInNewTab: (entry: Entry, handle: ListingHandle) => void;
}

/**
 * The entry is named by `(handle, id)` and Rust supplies its location, so a folder with a name
 * that is not valid UTF-8 opens as reliably as any other.
 */
export function useOpenEntry(navigation: Navigation): EntryOpeners {
	const client = useVfsClient();
	const tabs = useTabActions();
	const { goTo } = navigation;
	const { openInBackground } = tabs;
	return useMemo(() => {
		const resolve = (entry: Entry, handle: ListingHandle, then: (location: Location) => void) => {
			if (!isFolder(entry)) return;
			client.entryLocation(handle, entry.id).then(then, (error: unknown) => {
				console.warn('could not open the folder', error);
			});
		};
		return {
			open: (entry, handle) => resolve(entry, handle, goTo),
			openInNewTab: (entry, handle) => resolve(entry, handle, openInBackground),
		};
	}, [client, goTo, openInBackground]);
}
