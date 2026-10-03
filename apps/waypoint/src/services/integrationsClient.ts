// What the OS integrations can do here, as the Integrations page and the Services panel read it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IntegrationAvailability } from '@liminal-hq/waypoint-protocol/generated/IntegrationAvailability';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';

/**
 * The shared `xdg-portal` and `desktop-integration` plugins are used from Rust only (A65), so the
 * page asks the app's own commands what they can do. The answers are worked out in Rust from the
 * plugins' statuses; the page only shows them.
 */
export interface IntegrationsClient {
	/** What each switch can do here, with the reason where it cannot. */
	availability(): Promise<IntegrationAvailability>;
	/** The two plugins' statuses by their Services panel keys (`xdg-portal`, `desktop-integration`). */
	statuses(): Promise<Record<string, PluginStatus>>;
}
