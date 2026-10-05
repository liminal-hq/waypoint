// Verifies a window with the remote protocols turned off in Settings → Experimental: no Network section, a Connect dialog that lists only what is on, saved servers dimmed with the reason, and addresses that explain themselves
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeVfsClient } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, renderWorkspace } from '../test/workspaceHarness';
import { connectStore, openConnectDialog } from './connectStore';
import { draft, FakeConnectionsClient, serverLocation } from './fakeConnectionsClient';

const settingsWindow = vi.hoisted(() => ({ open: vi.fn() }));
vi.mock('../settings/openSettingsWindow', () => ({
	openSettingsWindow: settingsWindow.open,
	useSettingsShortcut: () => {},
}));

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
	settingsWindow.open.mockReset();
});
afterEach(() => {
	act(() => {
		connectStore.getState().answer(null);
		connectStore.getState().close();
	});
	cleanup();
	restoreLayout();
});

/** A fresh profile: the build has SFTP and WebDAV, and every switch is off. */
const allOff = (extra: ConstructorParameters<typeof FakeConnectionsClient>[0] = {}) =>
	new FakeConnectionsClient({ schemes: [], off: ['sftp', 'dav', 'davs'], ...extra });

const NAS = serverLocation('sftp://me@nas.lan');

describe('with every remote protocol turned off', () => {
	it('shows no Network section', async () => {
		await renderWorkspace(undefined, undefined, undefined, {
			sidebar: true,
			connections: allOff({ connections: [draft({ host: 'nas.lan', user: 'me', name: 'NAS' })] }),
		});
		await screen.findByRole('group', { name: 'Places' });
		await waitFor(() => expect(screen.queryByRole('group', { name: 'Network' })).toBeNull());
	});

	it('shows the section while a protocol is on, and brings it back without a reload when one is turned on', async () => {
		const connections = allOff();
		await renderWorkspace(undefined, undefined, undefined, { sidebar: true, connections });
		await screen.findByRole('group', { name: 'Places' });
		await waitFor(() => expect(screen.queryByRole('group', { name: 'Network' })).toBeNull());
		act(() => connections.setProtocols(['sftp']));
		expect(await screen.findByRole('group', { name: 'Network' })).toBeVisible();
		act(() => connections.setProtocols([]));
		await waitFor(() => expect(screen.queryByRole('group', { name: 'Network' })).toBeNull());
	});

	it('offers no Connect to Server in the section while a window has none to connect to', async () => {
		const connections = allOff();
		await renderWorkspace(undefined, undefined, undefined, { sidebar: true, connections });
		await screen.findByRole('group', { name: 'Places' });
		expect(screen.queryByRole('button', { name: 'Connect to Server…' })).toBeNull();
	});
});

describe('with only some protocols on', () => {
	it('dims a saved server of a protocol that is off, with the reason, and offers the page in its menu', async () => {
		const connections = new FakeConnectionsClient({
			schemes: ['sftp'],
			off: ['dav', 'davs'],
			connections: [
				draft({ host: 'nas.lan', user: 'me', name: 'NAS' }),
				draft({ scheme: 'davs', host: 'cloud.example', name: 'Cloud' }),
			],
		});
		await renderWorkspace(undefined, undefined, undefined, { sidebar: true, connections });
		const group = await screen.findByRole('group', { name: 'Network' });
		const cloud = await within(group).findByRole('button', { name: /Cloud/ });
		expect(cloud).toHaveAccessibleName(/Turned off in Settings → Experimental/);
		expect(cloud.closest('li')).toHaveAttribute('data-off');
		const nas = within(group).getByRole('button', { name: /NAS/ });
		expect(nas).toHaveAccessibleName(/Not connected/);
		expect(nas.closest('li')).not.toHaveAttribute('data-off');

		fireEvent.keyDown(cloud, { key: 'ContextMenu' });
		expect(screen.queryByRole('menuitem', { name: 'Connect' })).toBeNull();
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Open Experimental Settings…' }));
		expect(settingsWindow.open).toHaveBeenCalledWith('experimental');
	});

	const protocolOptions = (dialog: HTMLElement) =>
		within(within(dialog).getByRole('combobox', { name: 'Protocol' }))
			.getAllByRole('option')
			.map((option) => option.textContent);

	const partlyOff = () =>
		new FakeConnectionsClient({
			schemes: ['sftp'],
			off: ['dav', 'davs'],
			connections: [draft({ scheme: 'davs', host: 'cloud.example', name: 'Cloud' })],
		});

	it('lists only the protocols that are on in the Connect dialog', async () => {
		await renderWorkspace(undefined, new FakeTabsApi(), undefined, { connections: partlyOff() });
		act(() => openConnectDialog());
		const dialog = await screen.findByRole('dialog', { name: 'Connect to Server' });
		await waitFor(() => expect(protocolOptions(dialog)).toEqual(['SFTP (SSH)']));
	});

	it('keeps a saved server’s own protocol in the list when editing it, marked as turned off', async () => {
		await renderWorkspace(undefined, new FakeTabsApi(), undefined, { connections: partlyOff() });
		await waitFor(() => expect(connectStore.getState().dialog).toBeNull());
		act(() => openConnectDialog({ mode: 'edit', id: 'c1' }));
		const dialog = await screen.findByRole('dialog', { name: 'Edit Connection' });
		await waitFor(() =>
			expect(protocolOptions(dialog)).toEqual(['SFTP (SSH)', 'WebDAV (HTTPS) (turned off)']),
		);
	});

	it('says an address of a protocol that is off is turned off, and links to the page', async () => {
		const connections = new FakeConnectionsClient({ schemes: ['sftp'], off: ['davs'] });
		await renderWorkspace(undefined, new FakeTabsApi(), undefined, { connections });
		act(() => openConnectDialog());
		const dialog = await screen.findByRole('dialog', { name: 'Connect to Server' });
		fireEvent.change(within(dialog).getByRole('textbox', { name: 'Address' }), {
			target: { value: 'davs://cloud.example/files' },
		});
		expect(
			await within(dialog).findByText('WebDAV (HTTPS) is turned off in Settings → Experimental.'),
		).toBeVisible();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Open Experimental settings' }));
		expect(settingsWindow.open).toHaveBeenCalledWith('experimental');
	});

	it('keeps saying "cannot connect to those servers here" for a scheme the build never had', async () => {
		const connections = new FakeConnectionsClient({ schemes: ['sftp'], off: [] });
		await renderWorkspace(undefined, new FakeTabsApi(), undefined, { connections });
		act(() => openConnectDialog());
		const dialog = await screen.findByRole('dialog', { name: 'Connect to Server' });
		fireEvent.change(within(dialog).getByRole('textbox', { name: 'Address' }), {
			target: { value: 'smb://files/share' },
		});
		expect(await within(dialog).findByText(/cannot connect to smb servers here/)).toBeVisible();
		expect(within(dialog).queryByRole('button', { name: 'Open Experimental settings' })).toBeNull();
	});
});

describe('a location of a protocol that is off', () => {
	const off: VfsError = { kind: 'protocolOff', scheme: 'sftp' };

	async function openTab(client: FakeVfsClient, connections: FakeConnectionsClient) {
		const tabs = new FakeTabsApi();
		await tabs.openTab(NAS);
		await renderWorkspace(client, tabs, undefined, { connections });
	}

	it('opens in a tab as a clear reason with the link, never a blank view', async () => {
		const client = createTree();
		client.failOpening(NAS, off);
		await openTab(client, allOff());
		expect(await screen.findByRole('heading', { name: 'SFTP (SSH) is turned off' })).toBeVisible();
		expect(screen.getByRole('alert')).toHaveTextContent(/turn it on in Settings → Experimental/);
		fireEvent.click(screen.getByRole('button', { name: 'Open Experimental settings' }));
		expect(settingsWindow.open).toHaveBeenCalledWith('experimental');
	});

	it('is refused in the path bar with the reason and the link, and the bar stays open', async () => {
		const client = createTree();
		(client as unknown as { options: { offSchemes: string[] } }).options.offSchemes = ['sftp'];
		await renderWorkspace(client, undefined, undefined, { connections: allOff() });
		fireEvent.keyDown(window, { key: 'l', ctrlKey: true });
		const input = await screen.findByRole('textbox', { name: /location/i });
		fireEvent.change(input, { target: { value: 'sftp://me@nas.lan/srv' } });
		fireEvent.submit(input.closest('form')!);
		expect(
			await screen.findByText(
				/SFTP \(SSH\) is turned off\. You can turn it on in Settings → Experimental/,
			),
		).toBeVisible();
		fireEvent.click(screen.getByRole('button', { name: 'Open Experimental settings' }));
		expect(settingsWindow.open).toHaveBeenCalledWith('experimental');
		expect(screen.getByRole('textbox', { name: /location/i })).toBeVisible();
	});
});
