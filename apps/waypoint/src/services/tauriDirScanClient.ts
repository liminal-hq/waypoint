// The real DirScanClient: the directory-size scan and its cache from the waypoint-vfs plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as vfs from '@liminal-hq/waypoint-plugin-vfs';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { DirScanClient } from './dirScanClient';
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

/** A `DirScanClient` over the `waypoint-vfs` plugin. */
export function createTauriDirScanClient(): DirScanClient {
	return {
		scan: (location, onEvent, options) => call(vfs.scanDirSizes(location, onEvent, options)),
		cached: (location) => call(vfs.getCachedDirScan(location)),
	};
}
