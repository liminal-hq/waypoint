// The real GitClient: the waypoint-git plugin's commands through its guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as git from '@liminal-hq/waypoint-plugin-git';
import type { GitClient } from './gitClient';
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
		(error: unknown) => console.warn('could not listen for a Git change', error),
	);
	return () => {
		stopped = true;
		unlisten?.();
		unlisten = undefined;
	};
}

/** A `GitClient` over the plugin. */
export function createTauriGitClient(): GitClient {
	return {
		watch: (location) => git.gitWatch(location),
		unwatch: (id) => git.gitUnwatch(id),
		badges: (locations) => git.gitBadges(locations),
		onChanged: (listener) => subscribe(git.onGitChanged(listener)),
	};
}
