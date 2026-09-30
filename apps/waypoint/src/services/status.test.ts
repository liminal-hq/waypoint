// Tests for the plugin status service
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PluginStatus } from '../domain/protocol/generated/PluginStatus';
import { getPluginStatus } from './status';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('getPluginStatus', () => {
	beforeEach(() => {
		vi.mocked(invoke).mockReset();
	});

	it('invokes the plugin-scoped get_status command', async () => {
		const status: PluginStatus = { available: true, reason: null, features: ['trash'] };
		vi.mocked(invoke).mockResolvedValue(status);

		await expect(getPluginStatus('waypoint-vfs')).resolves.toEqual(status);
		expect(invoke).toHaveBeenCalledWith('plugin:waypoint-vfs|get_status');
	});
});
