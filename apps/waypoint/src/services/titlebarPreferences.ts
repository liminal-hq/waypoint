// Follows the OS titlebar preferences through the system-appearance plugin's guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	getTitlebarPreferences,
	onTitlebarPreferencesChanged,
	type TitlebarPreferences,
	type TitlebarSnapshot,
} from '@liminal-hq/plugin-system-appearance';
import { useEffect, useState } from 'react';

/**
 * The OS's titlebar preferences, or `null` until they have been read (and for good outside
 * Tauri, where the plugin is not there). Stays current: the plugin pushes changes when the
 * user edits their desktop settings.
 *
 * A change event and a read can arrive in either order, so every snapshot carries a revision
 * and only one newer than the current one is used. The hook subscribes before it reads, so a
 * change made in between reaches it as an event instead of falling through the gap.
 */
export function useTitlebarPreferences(): TitlebarPreferences | null {
	const [preferences, setPreferences] = useState<TitlebarPreferences | null>(null);

	useEffect(() => {
		let active = true;
		let unlisten: (() => void) | undefined;
		let revision = 0;

		const accept = (snapshot: TitlebarSnapshot) => {
			if (!active || snapshot.revision <= revision) return;
			revision = snapshot.revision;
			setPreferences(snapshot);
		};

		onTitlebarPreferencesChanged(accept)
			.then((off) => {
				if (!active) {
					off();
					return undefined;
				}
				unlisten = off;
				return getTitlebarPreferences().then(accept);
			})
			.catch(() => {
				// Not in Tauri, or the plugin is unavailable: the caller falls back to a default.
			});

		return () => {
			active = false;
			unlisten?.();
		};
	}, []);

	return preferences;
}
