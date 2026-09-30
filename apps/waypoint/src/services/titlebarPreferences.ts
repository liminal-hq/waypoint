// Follows the OS titlebar preferences through the system-appearance plugin's guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	getTitlebarPreferences,
	onTitlebarPreferencesChanged,
	type TitlebarPreferences,
} from '@liminal-hq/plugin-system-appearance';
import { useEffect, useState } from 'react';

/**
 * The OS's titlebar preferences, or `null` until they have been read (and for good outside
 * Tauri, where the plugin is not there). Stays current: the plugin pushes changes when the
 * user edits their desktop settings.
 */
export function useTitlebarPreferences(): TitlebarPreferences | null {
	const [preferences, setPreferences] = useState<TitlebarPreferences | null>(null);

	useEffect(() => {
		let active = true;
		let unlisten: (() => void) | undefined;

		getTitlebarPreferences()
			.then((value) => {
				if (active) setPreferences(value);
			})
			.catch(() => {
				// Not in Tauri, or the plugin is unavailable: the caller falls back to a default.
			});

		onTitlebarPreferencesChanged((value) => {
			if (active) setPreferences(value);
		})
			.then((off) => {
				if (active) unlisten = off;
				else off();
			})
			.catch(() => {});

		return () => {
			active = false;
			unlisten?.();
		};
	}, []);

	return preferences;
}
