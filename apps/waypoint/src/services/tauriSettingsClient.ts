// The real SettingsClient: the waypoint-settings plugin through its guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as settings from '@liminal-hq/waypoint-plugin-settings';
import type { SettingsClient } from './settingsClient';
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
		(error: unknown) => console.warn('could not listen for a settings change', error),
	);
	return () => {
		stopped = true;
		unlisten?.();
		unlisten = undefined;
	};
}

/** A `SettingsClient` over the plugin. */
export function createTauriSettingsClient(): SettingsClient {
	return {
		snapshot: () => settings.getSettings(),
		set: (next) => settings.setSettings(next),
		onChanged: (listener) => subscribe(settings.onSettingsChanged(listener)),
	};
}
