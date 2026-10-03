// The real FolderViewsClient: the waypoint-settings plugin's folder view commands through its guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as settings from '@liminal-hq/waypoint-plugin-settings';
import type { FolderViewsClient } from './folderViewsClient';
import type { Unsubscribe } from './vfsClient';

/** Turns a listener registration that resolves later into an unsubscribe that works at once. */
function subscribe(registration: Promise<() => void>): Unsubscribe {
	let unlisten: (() => void) | undefined;
	let stopped = false;
	void registration.then(
		(fn) => {
			if (stopped) fn();
			else unlisten = fn;
		},
		(error: unknown) => console.warn('could not listen for a folder view change', error),
	);
	return () => {
		stopped = true;
		unlisten?.();
		unlisten = undefined;
	};
}

/** A `FolderViewsClient` over the plugin. */
export function createTauriFolderViewsClient(): FolderViewsClient {
	return {
		snapshot: () => settings.getFolderViews(),
		remember: (key, patch) => settings.rememberFolderView(key, patch),
		reset: (key) => settings.resetFolderView(key),
		onChanged: (listener) => subscribe(settings.onFolderViewsChanged(listener)),
	};
}
