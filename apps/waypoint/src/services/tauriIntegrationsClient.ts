// The real IntegrationsClient: the app's `get_integration_availability` and `get_integration_statuses` commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { IntegrationAvailability } from '@liminal-hq/waypoint-protocol/generated/IntegrationAvailability';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import type { IntegrationsClient } from './integrationsClient';

/** An `IntegrationsClient` over the app's own commands. */
export function createTauriIntegrationsClient(): IntegrationsClient {
	return {
		availability: () => invoke<IntegrationAvailability>('get_integration_availability'),
		statuses: () => invoke<Record<string, PluginStatus>>('get_integration_statuses'),
	};
}
