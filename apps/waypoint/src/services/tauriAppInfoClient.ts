// The real AppInfoClient: the version Tauri reports for the application
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getVersion } from '@tauri-apps/api/app';
import type { AppInfoClient } from './appInfoClient';

/** An `AppInfoClient` over Tauri's `app` API. Create one per window. */
export function createTauriAppInfoClient(): AppInfoClient {
	return { getVersion };
}
