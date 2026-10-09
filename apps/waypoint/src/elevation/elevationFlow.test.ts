// Verifies asking for administrator rights: the prompt comes first, a new tab follows only on success, and a cancel or refusal leaves nothing behind
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { describe, expect, it, vi } from 'vitest';
import { FakeConnectionsClient } from '../connections/fakeConnectionsClient';
import { t } from '../i18n/messages';
import { fileLocation } from '../services/fakeVfsClient';
import {
	connectAdministrator,
	elevationFailureText,
	leaveAdministrator,
	openAsAdministrator,
} from './elevationFlow';
import { createElevationStore } from './elevationStore';

const ETC = fileLocation('/etc');
const ADMIN_ETC = { display: '/etc', uri: 'admin:///etc' };
const tab = (location = ETC): TabSnapshot => ({ id: 7, location }) as unknown as TabSnapshot;

function setup(client = new FakeConnectionsClient()) {
	const tabs = {
		openTab: vi.fn().mockResolvedValue(9),
		navigate: vi.fn().mockResolvedValue(undefined),
	};
	const say = vi.fn();
	const announce = vi.fn();
	return { client, tabs, say, announce, deps: { client, tabs, active: tab(), say, announce } };
}

describe('connectAdministrator', () => {
	it('connects the admin login with no answer and says it is waiting only while it waits', async () => {
		const store = createElevationStore();
		const client = new FakeConnectionsClient();
		client.holdConnects = true;
		const done = connectAdministrator(client, ADMIN_ETC, store);
		await vi.waitFor(() => expect(store.getState().pending).not.toBeNull());
		expect(client.calls.find((call) => call.method === 'connect')?.args[0]).toEqual(ADMIN_ETC);
		client.releaseConnects(null);
		expect(await done).toEqual({ kind: 'connected' });
		expect(store.getState().pending).toBeNull();
	});

	it('cancels through the client, and ends as cancelled with nothing pending', async () => {
		const store = createElevationStore();
		const client = new FakeConnectionsClient();
		client.holdConnects = true;
		const done = connectAdministrator(client, ADMIN_ETC, store);
		await vi.waitFor(() => expect(store.getState().pending).not.toBeNull());
		store.getState().pending?.cancel();
		expect(await done).toEqual({ kind: 'cancelled' });
		expect(client.calls.some((call) => call.method === 'cancelConnect')).toBe(true);
		expect(store.getState().pending).toBeNull();
	});

	it.each<[VfsError, string]>([
		[
			{ kind: 'authFailed', location: ADMIN_ETC, reason: 'refused' } as unknown as VfsError,
			'refused',
		],
		[{ kind: 'unsupported', what: 'polkit is not installed' }, 'failed'],
	])('ends as %j', async (error, kind) => {
		const client = new FakeConnectionsClient({ connect: () => error });
		const outcome = await connectAdministrator(client, ADMIN_ETC, createElevationStore());
		expect(outcome.kind).toBe(kind);
	});
});

describe('elevationFailureText', () => {
	it('says a refusal as cancelled or refused, since the system does not always tell them apart', () => {
		expect(elevationFailureText({ kind: 'refused' })).toBe(
			'Authentication was cancelled or refused',
		);
	});

	it('gives the reason of an unsupported helper', () => {
		expect(
			elevationFailureText({
				kind: 'failed',
				error: { kind: 'unsupported', what: 'The helper is not installed' },
			}),
		).toBe('Administrator access is not available: The helper is not installed');
	});

	it('says nothing for a cancel or a success', () => {
		expect(elevationFailureText({ kind: 'cancelled' })).toBeNull();
		expect(elevationFailureText({ kind: 'connected' })).toBeNull();
	});
});

describe('openAsAdministrator', () => {
	it('connects first, then opens a new tab beside the active one at the elevated folder', async () => {
		const { client, tabs, announce, deps } = setup();
		const order: string[] = [];
		tabs.openTab.mockImplementation(async () => {
			order.push('openTab');
			return 9;
		});
		const connect = client.connect.bind(client);
		client.connect = async (...args) => {
			order.push('connect');
			return connect(...args);
		};
		const outcome = await openAsAdministrator(deps, ETC);
		expect(outcome.kind).toBe('connected');
		expect(order).toEqual(['connect', 'openTab']);
		expect(tabs.openTab).toHaveBeenCalledWith(ADMIN_ETC, { after: 7 });
		expect(announce).toHaveBeenCalledWith('Opened /etc as an administrator');
	});

	it('opens nothing and says nothing when the person cancels', async () => {
		const { client, tabs, say, deps } = setup(
			new FakeConnectionsClient({ connect: () => ({ kind: 'cancelled' }) }),
		);
		const outcome = await openAsAdministrator(deps, ETC);
		expect(outcome.kind).toBe('cancelled');
		expect(client.calls.filter((call) => call.method === 'connect')).toHaveLength(1);
		expect(tabs.openTab).not.toHaveBeenCalled();
		expect(say).not.toHaveBeenCalled();
	});

	it('says "Authentication was cancelled or refused" and opens nothing when the system refuses', async () => {
		const refused = { kind: 'authFailed', location: ADMIN_ETC } as unknown as VfsError;
		const { tabs, say, deps } = setup(new FakeConnectionsClient({ connect: () => refused }));
		await openAsAdministrator(deps, ETC);
		expect(say).toHaveBeenCalledWith(t('elevation.refused'));
		expect(tabs.openTab).not.toHaveBeenCalled();
	});

	it('gives the reason when the helper cannot start here', async () => {
		const unsupported: VfsError = { kind: 'unsupported', what: 'running as an AppImage' };
		const { tabs, say, deps } = setup(new FakeConnectionsClient({ connect: () => unsupported }));
		await openAsAdministrator(deps, ETC);
		expect(say).toHaveBeenCalledWith(
			'Administrator access is not available: running as an AppImage',
		);
		expect(tabs.openTab).not.toHaveBeenCalled();
	});
});

describe('leaveAdministrator', () => {
	it('navigates the same tab to the ordinary form of its folder', async () => {
		const { tabs, announce } = setup();
		const active = { id: 3, location: ADMIN_ETC } as unknown as TabSnapshot;
		expect(await leaveAdministrator({ tabs, active, announce })).toBe(true);
		expect(tabs.navigate).toHaveBeenCalledWith(3, { display: '/etc', uri: 'file:///etc' });
		expect(announce).toHaveBeenCalledWith('Left Administrator Mode');
	});

	it('does nothing in a tab that is not elevated, or with no tab', async () => {
		const { tabs, announce } = setup();
		expect(await leaveAdministrator({ tabs, active: tab(), announce })).toBe(false);
		expect(await leaveAdministrator({ tabs, active: undefined, announce })).toBe(false);
		expect(tabs.navigate).not.toHaveBeenCalled();
	});
});
