// Tests for the real PlacesClient with the plugin's guest-js module mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { Places } from '@liminal-hq/waypoint-protocol/generated/Places';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriPlacesClient } from './tauriPlacesClient';
import { isVfsError } from './vfsClient';

const plugin = vi.hoisted(() => ({
	listPlaces: vi.fn(),
	addFavourite: vi.fn(),
	removeFavourite: vi.fn(),
	renameFavourite: vi.fn(),
	moveFavourite: vi.fn(),
}));
vi.mock('@liminal-hq/waypoint-plugin-vfs', () => plugin);

const folder: Location = { display: '/srv/a', uri: 'file:///srv/a' };
const places: Places = {
	places: [],
	favourites: [{ label: 'a', location: folder }],
};

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

describe('createTauriPlacesClient', () => {
	it('reads the places without announcing a change', async () => {
		const client = createTauriPlacesClient();
		const listener = vi.fn();
		client.onChange(listener);
		plugin.listPlaces.mockResolvedValue(places);
		expect(await client.list()).toBe(places);
		expect(listener).not.toHaveBeenCalled();
	});

	it('passes each mutation through and announces the places it resolves with', async () => {
		const client = createTauriPlacesClient();
		const listener = vi.fn();
		client.onChange(listener);
		for (const fn of [
			plugin.addFavourite,
			plugin.removeFavourite,
			plugin.renameFavourite,
			plugin.moveFavourite,
		]) {
			fn.mockResolvedValue(places);
		}
		await client.addFavourite(folder, 'x');
		expect(plugin.addFavourite).toHaveBeenCalledWith(folder, 'x');
		await client.removeFavourite(folder);
		expect(plugin.removeFavourite).toHaveBeenCalledWith(folder);
		await client.renameFavourite(folder, null);
		expect(plugin.renameFavourite).toHaveBeenCalledWith(folder, null);
		await client.moveFavourite(folder, 2);
		expect(plugin.moveFavourite).toHaveBeenCalledWith(folder, 2);
		expect(listener).toHaveBeenCalledTimes(4);
		expect(listener).toHaveBeenLastCalledWith(places);
	});

	it('rejects with the plugin error, wraps any other failure as a VfsError, and stays quiet', async () => {
		const client = createTauriPlacesClient();
		const listener = vi.fn();
		client.onChange(listener);
		const denied = { kind: 'permissionDenied', location: folder };
		plugin.addFavourite.mockRejectedValue(denied);
		await expect(client.addFavourite(folder)).rejects.toBe(denied);
		plugin.removeFavourite.mockRejectedValue('command not allowed');
		const error = await client.removeFavourite(folder).catch((e: unknown) => e);
		expect(isVfsError(error)).toBe(true);
		expect(error).toMatchObject({ kind: 'io', message: 'command not allowed' });
		expect(listener).not.toHaveBeenCalled();
	});

	it('stops announcing to a listener that unsubscribed', async () => {
		const client = createTauriPlacesClient();
		const listener = vi.fn();
		client.onChange(listener)();
		plugin.addFavourite.mockResolvedValue(places);
		await client.addFavourite(folder);
		expect(listener).not.toHaveBeenCalled();
	});
});
