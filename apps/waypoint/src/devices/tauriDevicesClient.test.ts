// Tests for the real DevicesClient with the volumes plugin's guest-js module mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriDevicesClient } from './tauriDevicesClient';

const plugin = vi.hoisted(() => ({
	getStatus: vi.fn(),
	list: vi.fn(),
	mount: vi.fn(),
	unmount: vi.fn(),
	eject: vi.fn(),
	unlock: vi.fn(),
	forget: vi.fn(),
	refreshSpace: vi.fn(),
	onChanged: vi.fn(),
}));
vi.mock('@liminal-hq/plugin-volumes', () => plugin);

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

describe('createTauriDevicesClient', () => {
	it('passes the actions to the plugin', async () => {
		plugin.mount.mockResolvedValue('/run/media/a/Backup');
		plugin.unlock.mockResolvedValue({ id: 'vol-2', remember: { state: 'notAsked' } });
		plugin.forget.mockResolvedValue(true);
		const client = createTauriDevicesClient();
		expect(await client.mount('vol-1')).toBe('/run/media/a/Backup');
		await client.unmount('vol-1');
		await client.eject('vol-1');
		expect(await client.unlock('vol-3', 'secret', true)).toEqual({
			id: 'vol-2',
			remember: { state: 'notAsked' },
		});
		expect(await client.forget('vol-3')).toBe(true);
		expect(plugin.unmount).toHaveBeenCalledWith('vol-1');
		expect(plugin.eject).toHaveBeenCalledWith('vol-1');
		expect(plugin.unlock).toHaveBeenCalledWith('vol-3', 'secret', true);
		expect(plugin.forget).toHaveBeenCalledWith('vol-3');
	});

	it('measures one volume on request', async () => {
		plugin.refreshSpace.mockResolvedValue({ id: 'nas', total: 10, free: 4 });
		expect(await createTauriDevicesClient().refreshSpace('nas')).toEqual({
			id: 'nas',
			total: 10,
			free: 4,
		});
		expect(plugin.refreshSpace).toHaveBeenCalledWith('nas');
	});

	it('lists without measuring network volumes', async () => {
		plugin.list.mockResolvedValue([]);
		await createTauriDevicesClient().list();
		expect(plugin.list).toHaveBeenCalledWith();
	});

	it('stops hearing changes once unsubscribed, even before the listener was registered', async () => {
		let registered: ((event: unknown) => void) | undefined;
		const stop = vi.fn();
		plugin.onChanged.mockImplementation((handler: (event: unknown) => void) => {
			registered = handler;
			return Promise.resolve(stop);
		});
		const heard = vi.fn();
		const unsubscribe = createTauriDevicesClient().onChanged(heard);
		registered?.({ revision: 1, volumes: [] });
		expect(heard).toHaveBeenCalledTimes(1);
		unsubscribe();
		await Promise.resolve();
		registered?.({ revision: 2, volumes: [] });
		expect(heard).toHaveBeenCalledTimes(1);
		expect(stop).toHaveBeenCalled();
	});
});
