// The real PlacesClient: serves the sidebar from the waypoint-vfs plugin through its guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as vfs from '@liminal-hq/waypoint-plugin-vfs';
import type { Places } from '@liminal-hq/waypoint-protocol/generated/Places';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { PlacesChangeEmitter, type PlacesClient } from './placesClient';
import { isVfsError } from './vfsClient';

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

/** A `PlacesClient` over the `waypoint-vfs` plugin. Create one per window, not per view. */
export function createTauriPlacesClient(): PlacesClient {
	const changes = new PlacesChangeEmitter();
	const mutate = async (work: Promise<Places>) => changes.emit(await call(work));
	return {
		list: () => call(vfs.listPlaces()),
		addFavourite: (location, label) => mutate(vfs.addFavourite(location, label)),
		removeFavourite: (location) => mutate(vfs.removeFavourite(location)),
		renameFavourite: (location, label) => mutate(vfs.renameFavourite(location, label)),
		moveFavourite: (location, to) => mutate(vfs.moveFavourite(location, to)),
		onChange: (listener) => changes.subscribe(listener),
	};
}
