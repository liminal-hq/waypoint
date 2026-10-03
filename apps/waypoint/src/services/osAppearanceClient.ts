// The OS appearance preferences as the theme uses them: read once, and hear every change
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AppearancePreferences } from '@liminal-hq/plugin-system-appearance';
import type { Unsubscribe } from './vfsClient';

export type { AppearancePreferences };

export interface OsAppearanceClient {
	/** Reads the preferences now; rejects where the plugin is not there or the call fails. */
	get(): Promise<AppearancePreferences>;
	/** Calls `listener` with each change (every one carries a rising revision). */
	onChanged(listener: (preferences: AppearancePreferences) => void): Unsubscribe;
}
