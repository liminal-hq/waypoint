// Tests for the real ConnectionsClient with the file system plugin's guest-js module mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriConnectionsClient } from './tauriConnectionsClient';

const plugin = vi.hoisted(() => ({
	listConnections: vi.fn(),
	connectionSupport: vi.fn(),
	suggestedServers: vi.fn(),
	parseAddress: vi.fn(),
	addConnection: vi.fn(),
	updateConnection: vi.fn(),
	duplicateConnection: vi.fn(),
	removeConnection: vi.fn(),
	moveConnection: vi.fn(),
	forgetRecentServer: vi.fn(),
	forgetLogin: vi.fn(),
	connect: vi.fn(),
	testConnection: vi.fn(),
	disconnect: vi.fn(),
	connectionState: vi.fn(),
	onConnectionsChanged: vi.fn(),
	onConnectionState: vi.fn(),
}));
vi.mock('@liminal-hq/waypoint-plugin-vfs', () => plugin);

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

const here = { display: 'sftp://a/', uri: 'sftp://a/' };

describe('createTauriConnectionsClient', () => {
	it('passes answers once and defaults to no answer and no remembering', async () => {
		plugin.connect.mockResolvedValue({ kind: 'no' });
		const client = createTauriConnectionsClient();
		await client.connect(here);
		expect(plugin.connect).toHaveBeenCalledWith(here, null, false);
		await client.connect(here, { kind: 'passphrase', passphrase: 'x' }, true);
		expect(plugin.connect).toHaveBeenLastCalledWith(
			here,
			{ kind: 'passphrase', passphrase: 'x' },
			true,
		);
		await client.remove('c1', true);
		expect(plugin.removeConnection).toHaveBeenCalledWith('c1', true);
	});

	it('stops a subscription that resolves after it was stopped', async () => {
		const stop = vi.fn();
		let resolve: (value: () => void) => void = () => {};
		plugin.onConnectionState.mockReturnValue(new Promise((r) => (resolve = r)));
		const unsubscribe = createTauriConnectionsClient().onState(() => {});
		unsubscribe();
		resolve(stop);
		await Promise.resolve();
		await Promise.resolve();
		expect(stop).toHaveBeenCalled();
	});
});
