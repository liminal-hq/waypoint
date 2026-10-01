// What opening an entry of a listing means: a folder navigates or opens a tab, a file opens in its default app
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { useMemo } from 'react';
import { useVfsClient } from '../browse/VfsClientContext';
import { usePlacesClient } from '../sidebar/PlacesClientContext';
import { useTabActions } from '../tabs/tabActions';
import { useWindowActions } from '../tabs/windowActions';
import type { Navigation } from './useNavigation';

/** Whether opening `entry` goes into it: a folder, or a link that resolves to one. */
export function isFolder(entry: Entry): boolean {
	return entry.kind === 'directory' || entry.linkTarget === 'directory';
}

/** What was being attempted when `onFailure` is called, so each action can say so in its own words. */
export type EntryAction = 'open' | 'copyPath' | 'favourite';

/** What each action was doing, for the log line of a failure. */
const ACTION_TEXT: Record<EntryAction, string> = {
	open: 'open',
	copyPath: 'copy the path of',
	favourite: 'add to the favourites',
};

/** Stable, so omitting `onFailure` does not rebuild the openers every render. */
const ignoreFailure = (): void => {};

export interface EntryOpeners {
	/** Enter and double-click: a folder navigates the tab, a file opens in its default application. */
	open: (entry: Entry, handle: ListingHandle) => void;
	/**
	 * Middle-click and the menu: a folder opens in a tab beside this one, or in a new window when
	 * `inNewWindow` (Ctrl+middle-click), and anything else does nothing.
	 */
	openInNewTab: (entry: Entry, handle: ListingHandle, inNewWindow?: boolean) => void;
	/** Puts the entry's path, as Rust displays it, on the clipboard. */
	copyPath: (entry: Entry, handle: ListingHandle) => void;
	/** Pins a folder to the Favourites; anything else does nothing. */
	addToFavourites: (entry: Entry, handle: ListingHandle) => void;
}

/**
 * The entry is named by `(handle, id)` and Rust supplies its location or opens it, so a name that
 * is not valid UTF-8 works as reliably as any other. `onFailure` hears what could not be done, by
 * the entry's display name, for the status bar to show.
 */
export function useOpenEntry(
	navigation: Navigation,
	onFailure: (entry: Entry, action: EntryAction) => void = ignoreFailure,
): EntryOpeners {
	const client = useVfsClient();
	const places = usePlacesClient();
	const tabs = useTabActions();
	const { goTo } = navigation;
	const { openInBackground } = tabs;
	const { openInNewWindow } = useWindowActions();
	return useMemo(() => {
		const fail = (entry: Entry, action: EntryAction) => (error: unknown) => {
			console.warn(`could not ${ACTION_TEXT[action]} the entry`, error);
			onFailure(entry, action);
		};
		return {
			open: (entry, handle) => {
				if (isFolder(entry)) {
					client.entryLocation(handle, entry.id).then(goTo, fail(entry, 'open'));
				} else {
					client.openEntry(handle, entry.id).catch(fail(entry, 'open'));
				}
			},
			openInNewTab: (entry, handle, inNewWindow = false) => {
				if (isFolder(entry)) {
					client
						.entryLocation(handle, entry.id)
						.then(inNewWindow ? openInNewWindow : openInBackground, fail(entry, 'open'));
				}
			},
			copyPath: (entry, handle) => {
				client
					.entryLocation(handle, entry.id)
					.then((location) => navigator.clipboard.writeText(location.display))
					.catch(fail(entry, 'copyPath'));
			},
			addToFavourites: (entry, handle) => {
				if (!isFolder(entry)) return;
				client
					.entryLocation(handle, entry.id)
					.then((location) => places.addFavourite(location))
					.catch(fail(entry, 'favourite'));
			},
		};
	}, [client, places, goTo, openInBackground, openInNewWindow, onFailure]);
}
