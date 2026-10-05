// The app's `get_protocol_details` command: what each remote protocol this build includes can do
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';

/**
 * The logins and protections each remote protocol of this build supports, by protocol (`smb`,
 * `webdav`), with the provider's own reason for the first that does not work. A protocol the build
 * leaves out is not listed.
 */
export function protocolDetails(): Promise<Record<string, PluginStatus>> {
	return invoke<Record<string, PluginStatus>>('get_protocol_details');
}
