// What opening an entry of a listing means: a folder navigates or opens a tab, a file opens in its default app
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { useMemo } from 'react';
import { useVfsClient } from '../browse/VfsClientContext';
import { useTabActions } from '../tabs/tabActions';
import type { Navigation } from './useNavigation';

/** Whether opening `entry` goes into it: a folder, or a link that resolves to one. */
export function isFolder(entry: Entry): boolean {
	return entry.kind === 'directory' || entry.linkTarget === 'directory';
}

export interface EntryOpeners {
	/** Enter and double-click: a folder navigates the tab, a file opens in its default application. */
	open: (entry: Entry, handle: ListingHandle) => void;
	/** Middle-click and the menu: a folder opens in a tab beside this one, and anything else does nothing. */
	openInNewTab: (entry: Entry, handle: ListingHandle) => void;
	/** Puts the entry's path, as Rust displays it, on the clipboard. */
	copyPath: (entry: Entry, handle: ListingHandle) => void;
}

/**
 * The entry is named by `(handle, id)` and Rust supplies its location or opens it, so a name that
 * is not valid UTF-8 works as reliably as any other. `onFailure` hears what could not be done, by
 * the entry's display name, for the status bar to show.
 */
export function useOpenEntry(
	navigation: Navigation,
	onFailure: (entry: Entry) => void = () => {},
): EntryOpeners {
	const client = useVfsClient();
	const tabs = useTabActions();
	const { goTo } = navigation;
	const { openInBackground } = tabs;
	return useMemo(() => {
		const fail = (entry: Entry) => (error: unknown) => {
			console.warn('could not open the entry', error);
			onFailure(entry);
		};
		return {
			open: (entry, handle) => {
				if (isFolder(entry)) {
					client.entryLocation(handle, entry.id).then(goTo, fail(entry));
				} else {
					client.openEntry(handle, entry.id).catch(fail(entry));
				}
			},
			openInNewTab: (entry, handle) => {
				if (isFolder(entry)) {
					client.entryLocation(handle, entry.id).then(openInBackground, fail(entry));
				}
			},
			copyPath: (entry, handle) => {
				client
					.entryLocation(handle, entry.id)
					.then((location) => navigator.clipboard.writeText(location.display))
					.catch(fail(entry));
			},
		};
	}, [client, goTo, openInBackground, onFailure]);
}
