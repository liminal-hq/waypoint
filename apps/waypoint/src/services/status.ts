// Typed wrapper over a native plugin's get_status command
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { PluginStatus } from '../domain/protocol/generated/PluginStatus';

/** Asks the named plugin (for example `waypoint-vfs`) whether it works here. */
export function getPluginStatus(plugin: string): Promise<PluginStatus> {
	return invoke<PluginStatus>(`plugin:${plugin}|get_status`);
}
