// Reads the experimental native-context-menus switch, for the menu host and the pages that explain it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSettings } from '../settings/SettingsContext';
import type { Settings } from '../services/settingsClient';

const selectNativeContextMenus = (settings: Settings): boolean =>
	settings.experimental.nativeContextMenus;

/** Whether Settings → Experimental has native context menus on. Off until the person turns it on (D196). */
export function useNativeContextMenusSetting(): boolean {
	return useSettings(selectNativeContextMenus);
}
