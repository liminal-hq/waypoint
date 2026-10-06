// The real VfsClient: serves the file views from the waypoint-vfs plugin through its guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as vfs from '@liminal-hq/waypoint-plugin-vfs';
import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { isVfsError, type Unsubscribe, type VfsClient } from './vfsClient';

/**
 * Makes sure a rejection is a `VfsError`. The plugin rejects with the tagged object, so this only
 * matters for a failure outside the plugin's control (the command being denied, the IPC bridge
 * missing), which would otherwise reach the views as a bare string.
 */
function asVfsError(error: unknown): VfsError {
	if (isVfsError(error)) return error;
	const message = error instanceof Error ? error.message : String(error);
	return { kind: 'io', message, location: null };
}

async function call<T>(work: Promise<T>): Promise<T> {
	try {
		return await work;
	} catch (error) {
		throw asVfsError(error);
	}
}

/** A `VfsClient` over the `waypoint-vfs` plugin. Create one per app, not per view. */
export function createTauriVfsClient(): VfsClient {
	return {
		openListing: (location, options) => call(vfs.openListing(location, options)),
		getRange: (handle, start, count) => call(vfs.getRange(handle, start, count)),
		setSort: (handle, sort) => call(vfs.setSort(handle, sort)),
		setFilter: (handle, filter) => call(vfs.setFilter(handle, filter)),
		refreshListing: (handle, options) => call(vfs.refreshListing(handle, options)),
		closeListing: (handle) => call(vfs.closeListing(handle)),
		parseLocation: (input, base) => call(vfs.parseLocation(input, base)),
		parseLocationText: (input, base) => call(vfs.parseLocationText(input, base)),
		describeLocation: (location) => call(vfs.describeLocation(location)),
		entryLocation: (handle, id) => call(vfs.entryLocation(handle, id)),
		summariseSelection: (handle, selection) => call(vfs.summariseSelection(handle, selection)),
		getFreeSpace: (location) => call(vfs.getFreeSpace(location)),
		checkFolder: (location) => call(vfs.checkFolder(location)),
		openEntry: (handle, id) => call(vfs.openEntry(handle, id)),
		onListingEvent(listener: (event: ListingEvent) => void): Unsubscribe {
			let stopped = false;
			let unlisten: (() => void) | undefined;
			vfs
				.onListingEvent((event) => {
					if (!stopped) listener(event);
				})
				.then(
					(stop) => {
						// Unsubscribed before the listener was even registered: undo it straight away.
						if (stopped) stop();
						else unlisten = stop;
					},
					(error) => console.error('could not listen for listing events', error),
				);
			return () => {
				stopped = true;
				unlisten?.();
				unlisten = undefined;
			};
		},
	};
}
