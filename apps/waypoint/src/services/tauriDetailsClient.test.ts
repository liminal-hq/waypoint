// Tests for the real DetailsClient with the plugin's guest-js module mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriDetailsClient } from './tauriDetailsClient';
import { isVfsError } from './vfsClient';

const plugin = vi.hoisted(() => ({
	entryDetails: vi.fn(),
	folderSize: vi.fn(),
	readTextHead: vi.fn(),
	previewUrl: vi.fn(),
}));
vi.mock('@liminal-hq/waypoint-plugin-vfs', () => plugin);

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

describe('createTauriDetailsClient', () => {
	it('passes each call through to the plugin', async () => {
		const client = createTauriDetailsClient();
		plugin.entryDetails.mockResolvedValue({ name: 'a' });
		expect(await client.entryDetails(1, 2)).toEqual({ name: 'a' });
		expect(plugin.entryDetails).toHaveBeenCalledWith(1, 2);

		const onEvent = vi.fn();
		const run = { job: 4, cancel: vi.fn() };
		plugin.folderSize.mockResolvedValue(run);
		expect(await client.folderSize(1, 2, onEvent)).toBe(run);
		expect(plugin.folderSize).toHaveBeenCalledWith(1, 2, onEvent);

		plugin.readTextHead.mockResolvedValue({ text: 'x' });
		await client.readTextHead(1, 2, 100);
		expect(plugin.readTextHead).toHaveBeenCalledWith(1, 2, 100);

		plugin.previewUrl.mockReturnValue('wpfile://localhost/1-2');
		expect(client.previewUrl(1, 2)).toBe('wpfile://localhost/1-2');
	});

	it('rejects with the plugin error, and wraps any other failure as a VfsError', async () => {
		const client = createTauriDetailsClient();
		const binary = { kind: 'notText', location: { display: '/b', uri: 'file:///b' } };
		plugin.readTextHead.mockRejectedValue(binary);
		await expect(client.readTextHead(1, 2)).rejects.toBe(binary);
		plugin.entryDetails.mockRejectedValue('command not allowed');
		const error = await client.entryDetails(1, 2).catch((e: unknown) => e);
		expect(isVfsError(error)).toBe(true);
		expect(error).toMatchObject({ kind: 'io', message: 'command not allowed' });
	});
});
