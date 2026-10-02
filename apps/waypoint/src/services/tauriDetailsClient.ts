// The real DetailsClient: entry details, folder sizes and previews from the waypoint-vfs plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as vfs from '@liminal-hq/waypoint-plugin-vfs';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { DetailsClient } from './detailsClient';
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

/** A `DetailsClient` over the `waypoint-vfs` plugin. */
export function createTauriDetailsClient(): DetailsClient {
	return {
		entryDetails: (handle, id) => call(vfs.entryDetails(handle, id)),
		folderSize: (handle, id, onEvent) => call(vfs.folderSize(handle, id, onEvent)),
		readTextHead: (handle, id, max) => call(vfs.readTextHead(handle, id, max)),
		previewUrl: (handle, id) => vfs.previewUrl(handle, id),
	};
}
