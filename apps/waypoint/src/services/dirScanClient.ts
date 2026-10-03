// The frontend's view of the directory-size scan: biggest top-level folders of a root, with a cache
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { DirScanEvent } from '@liminal-hq/waypoint-protocol/generated/DirScanEvent';
import type { DirScanOptions } from '@liminal-hq/waypoint-protocol/generated/DirScanOptions';
import type { DirScanResult } from '@liminal-hq/waypoint-protocol/generated/DirScanResult';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';

/** A running directory-size scan. */
export interface DirScanJob {
	/** The run's id. */
	readonly job: number;
	/** Stops the scan; it ends with a `cancelled` event carrying the folders finished so far. */
	cancel(): Promise<void>;
}

/**
 * What Overview (and the future Disk usage view) need to measure where a root's space went. The
 * scan is Rust's: low priority, one top-level folder at a time, never across a mount point or a
 * symlink, and never downloading a cloud placeholder. `TauriDirScanClient` wraps the
 * `waypoint-vfs` plugin; `FakeDirScanClient` serves the same contract from memory.
 */
export interface DirScanClient {
	/**
	 * Starts a scan and resolves once it has started. `onEvent` gets `progress`, a `partial` result
	 * after each top-level folder, and then exactly one `done`, `cancelled` or `failed`. Rejects
	 * with a `VfsError` when it cannot start.
	 */
	scan(
		location: Location,
		onEvent: (event: DirScanEvent) => void,
		options?: DirScanOptions,
	): Promise<DirScanJob>;
	/**
	 * The last finished scan of `location`, with `measuredAtMs` for "as of <time>", or `null`
	 * when there is none.
	 */
	cached(location: Location): Promise<DirScanResult | null>;
}
