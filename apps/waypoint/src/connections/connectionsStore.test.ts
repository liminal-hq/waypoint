// Tests for following Rust's connections from one window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { stateOf } from './connectionsModel';
import { startConnections } from './connectionsStore';
import { draft, FakeConnectionsClient, serverLocation } from './fakeConnectionsClient';

describe('startConnections', () => {
	it('reads the overview and then follows edits and states', async () => {
		const client = new FakeConnectionsClient({
			connections: [draft({ host: 'a.lan', user: 'me' })],
		});
		const connections = startConnections(client);
		await connections.reload();
		expect(connections.store.getState().connections).toHaveLength(1);
		await client.add(draft({ host: 'b.lan' }));
		expect(connections.store.getState().connections.map((e) => e.label)).toEqual([
			'me@a.lan',
			'b.lan',
		]);
		await client.connect(serverLocation('sftp://b.lan'));
		expect(stateOf(connections.store.getState(), 'sftp://b.lan')).toEqual({ kind: 'connected' });
		connections.stop();
		await client.add(draft({ host: 'c.lan' }));
		expect(connections.store.getState().connections).toHaveLength(2);
	});

	it('sees a failed login as a state with its reason', async () => {
		const client = new FakeConnectionsClient({
			connect: () => ({
				kind: 'authRequired',
				location: serverLocation('sftp://a.lan'),
				prompt: { kind: 'password', user: null },
			}),
		});
		const connections = startConnections(client);
		await expect(client.connect(serverLocation('sftp://a.lan'))).rejects.toMatchObject({
			kind: 'authRequired',
		});
		const state = stateOf(connections.store.getState(), 'sftp://a.lan');
		expect(state.kind === 'failed' && state.error.kind).toBe('authRequired');
		connections.stop();
	});

	it('reads which protocols are on, then follows the switches without a reload', async () => {
		const client = new FakeConnectionsClient({ schemes: [], off: ['sftp'] });
		const connections = startConnections(client);
		await connections.reload();
		await vi.waitFor(() =>
			expect(connections.store.getState().protocols).toEqual({ schemes: [], off: ['sftp'] }),
		);
		client.setProtocols(['sftp']);
		expect(connections.store.getState().protocols).toEqual({ schemes: ['sftp'], off: [] });
		connections.stop();
		client.setProtocols([]);
		expect(connections.store.getState().protocols).toEqual({ schemes: ['sftp'], off: [] });
	});

	it('prefers a change heard before the first read finished', async () => {
		const client = new FakeConnectionsClient({ schemes: [], off: ['sftp'] });
		const slow = client.support.bind(client);
		client.support = async () => {
			const stale = await slow();
			// The switch is turned on while the read is in flight.
			client.setProtocols(['sftp']);
			return stale;
		};
		const connections = startConnections(client);
		await connections.reload();
		await vi.waitFor(() =>
			expect(connections.store.getState().protocols).toEqual({ schemes: ['sftp'], off: [] }),
		);
		connections.stop();
	});
});
