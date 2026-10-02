// Tests for the real DirScanClient with the plugin's guest-js module mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriDirScanClient } from './tauriDirScanClient';
import { isVfsError } from './vfsClient';

const plugin = vi.hoisted(() => ({
	scanDirSizes: vi.fn(),
	getCachedDirScan: vi.fn(),
}));
vi.mock('@liminal-hq/waypoint-plugin-vfs', () => plugin);

const home = { display: '/home/a', uri: 'file:///home/a' };

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

describe('createTauriDirScanClient', () => {
	it('passes each call through to the plugin', async () => {
		const client = createTauriDirScanClient();
		const onEvent = vi.fn();
		const run = { job: 3, cancel: vi.fn() };
		plugin.scanDirSizes.mockResolvedValue(run);
		expect(await client.scan(home, onEvent, { allocated: true })).toBe(run);
		expect(plugin.scanDirSizes).toHaveBeenCalledWith(home, onEvent, { allocated: true });

		plugin.getCachedDirScan.mockResolvedValue(null);
		expect(await client.cached(home)).toBeNull();
		expect(plugin.getCachedDirScan).toHaveBeenCalledWith(home);
	});

	it('rejects with the plugin error, and wraps any other failure as a VfsError', async () => {
		const client = createTauriDirScanClient();
		const missing = { kind: 'notFound', location: home };
		plugin.scanDirSizes.mockRejectedValue(missing);
		await expect(client.scan(home, vi.fn())).rejects.toBe(missing);
		plugin.getCachedDirScan.mockRejectedValue('command not allowed');
		const error = await client.cached(home).catch((e: unknown) => e);
		expect(isVfsError(error)).toBe(true);
		expect(error).toMatchObject({ kind: 'io', message: 'command not allowed' });
	});
});
