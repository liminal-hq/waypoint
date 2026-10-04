// What the browsing window asks of the Git plugin: watch a folder's repository, hear it change, ask for sidebar badges
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GitBadge } from '@liminal-hq/waypoint-protocol/generated/GitBadge';
import type { GitPathInfo } from '@liminal-hq/waypoint-protocol/generated/GitPathInfo';
import type { GitChanged } from '@liminal-hq/waypoint-protocol/generated/GitChanged';
import type { GitWatch } from '@liminal-hq/waypoint-protocol/generated/GitWatch';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { Unsubscribe } from './vfsClient';

export type { GitBadge, GitChanged, GitPathInfo, GitWatch };

/**
 * Rust reads the repositories; this window renders what it is told. `watch` starts hearing about
 * the repository that holds a folder and replies with where it stands (or `null` outside a working
 * tree, or when Git is switched off); `onChanged` delivers every later state. The marks on a
 * listing's rows are not here: they arrive with the listing itself.
 */
export interface GitClient {
	watch(location: Location): Promise<GitWatch | null>;
	unwatch(id: number): Promise<void>;
	/** How many paths changed inside each of `locations` that is in a working tree; the rest have no entry. */
	badges(locations: Location[]): Promise<GitBadge[]>;
	/** The newest commits that changed a file or folder and how much of it changed since `HEAD`; `null` outside a working tree or with Git off. Walks history, so ask once a selection settles. */
	pathInfo(location: Location, limit?: number): Promise<GitPathInfo | null>;
	onChanged(listener: (changed: GitChanged) => void): Unsubscribe;
}
