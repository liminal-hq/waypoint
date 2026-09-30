// Exposes typed guest-side wrappers for the waypoint-vfs plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';

const PREFIX = 'plugin:waypoint-vfs|';

/** Reports whether the file system plugin can do anything yet, and which features work. */
export function getStatus(): Promise<PluginStatus> {
	return invoke<PluginStatus>(`${PREFIX}get_status`);
}
