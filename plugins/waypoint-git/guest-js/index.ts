// Exposes typed guest-side wrappers for the waypoint-git plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { GitBadge } from '@liminal-hq/waypoint-protocol/generated/GitBadge';
import type { GitChanged } from '@liminal-hq/waypoint-protocol/generated/GitChanged';
import type { GitWatch } from '@liminal-hq/waypoint-protocol/generated/GitWatch';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';

const PREFIX = 'plugin:waypoint-git|';

/** Sent to the window that watches a repository when its state changes; the payload is a `GitChanged`. */
export const GIT_EVENT = 'waypoint-git://changed';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** What the plugin can do here; unavailable, with the reason, while the Settings switch has Git off. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/**
 * Starts hearing about the repository that holds `location` and replies with where it stands, or
 * `null` when the folder is not in a working tree, is not a local folder, or Git is off. Changes
 * follow as `GIT_EVENT`; read this reply first and apply events whose `id` is the watch's and whose
 * revision is higher.
 */
export function gitWatch(location: Location): Promise<GitWatch | null> {
	return cmd<GitWatch | null>('git_watch', { location });
}

/** Stops one watch. */
export function gitUnwatch(id: number): Promise<void> {
	return cmd<void>('git_unwatch', { id });
}

/** How many paths changed inside each of `locations` that is a folder in a working tree; the others have no entry. */
export function gitBadges(locations: Location[]): Promise<GitBadge[]> {
	return cmd<GitBadge[]>('git_badges', { locations });
}

/** Hears every change of the repositories this window watches. */
export function onGitChanged(listener: (changed: GitChanged) => void): Promise<UnlistenFn> {
	return listen<GitChanged>(GIT_EVENT, (e) => listener(e.payload));
}
