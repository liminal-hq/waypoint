// A DefaultFileManagerClient whose answers a test sets, recording what it was asked to do
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	CurrentFileManager,
	DefaultFileManagerClient,
	FileManagerAction,
} from './defaultFileManagerClient';

export interface FakeDefaultFileManager extends DefaultFileManagerClient {
	/** How many times `make` was called. */
	readonly made: number;
	/** What `current` answers next. */
	setCurrent(current: CurrentFileManager | null): void;
}

/**
 * A client for `action`. `make` makes Waypoint the default when the action is `set`, rejects with
 * `makeFails` when that is given, and only counts the call otherwise.
 */
export function createFakeDefaultFileManagerClient(
	action: FileManagerAction = { kind: 'set' },
	initial: CurrentFileManager | null = { isWaypoint: false, name: 'Files' },
	makeFails?: string,
): FakeDefaultFileManager {
	let current = initial;
	let made = 0;
	return {
		get made() {
			return made;
		},
		setCurrent(next) {
			current = next;
		},
		async action() {
			return action;
		},
		async current() {
			return current;
		},
		async make() {
			made += 1;
			if (makeFails) throw new Error(makeFails);
			if (action.kind === 'set') current = { isWaypoint: true, name: null };
		},
	};
}
