// The real SettingsTransferClient: the waypoint-settings plugin's export and import commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as settings from '@liminal-hq/waypoint-plugin-settings';
import type { SettingsTransferClient } from './settingsTransferClient';

/** A `SettingsTransferClient` over the plugin. */
export function createTauriSettingsTransferClient(): SettingsTransferClient {
	return {
		// The offset puts the person's own date in the suggested file name.
		exportSettings: () => settings.exportSettings(-new Date().getTimezoneOffset()),
		planImport: () => settings.planSettingsImport(),
		applyImport: (planId) => settings.applySettingsImport(planId),
	};
}
