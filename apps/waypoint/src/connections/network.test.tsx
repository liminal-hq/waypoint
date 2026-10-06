// Verifies the Network section, remote tab states and server addresses in the path bar against fake services
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, renderWorkspace } from '../test/workspaceHarness';
import { connectStore } from './connectStore';
import { draft, FakeConnectionsClient, serverLocation } from './fakeConnectionsClient';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	act(() => {
		connectStore.getState().answer(null);
		connectStore.getState().close();
	});
	cleanup();
	restoreLayout();
});

const NAS = serverLocation('sftp://me@nas.lan');

/** The tree, with a server folder the fake can open. */
function treeWithServer(): FakeVfsClient {
	const client = createTree();
	(client as unknown as { options: { remoteSchemes: string[] } }).options.remoteSchemes = ['sftp'];
	client.setFolder(NAS, [makeEntry(1, 'media', { kind: 'directory' })]);
	return client;
}

const network = () => screen.findByRole('group', { name: 'Network' });
const sidebarStatus = () =>
	within(screen.getByRole('navigation', { name: 'Sidebar' })).getByRole('status');

describe('the Network section', () => {
	it('lists saved and recent servers with their state in words, and announces each change', async () => {
		const connections = new FakeConnectionsClient({
			connections: [draft({ host: 'nas.lan', user: 'me', name: 'NAS' })],
			recent: [{ key: 'sftp://pi', location: serverLocation('sftp://pi'), atMs: 1 }],
		});
		await renderWorkspace(undefined, undefined, undefined, { sidebar: true, connections });
		const group = await network();
		const nas = await within(group).findByRole('button', { name: /NAS.*Not connected/ });
		expect(
			within(group).getByRole('button', { name: /sftp:\/\/pi\/.*Not connected/ }),
		).toBeVisible();
		// A server row takes files dropped on it, into the folder it opens at (D151).
		expect(nas).toHaveAttribute('data-drop', 'place');
		expect(nas.getAttribute('data-drop-ref')).toMatch(/^sftp:\/\/me@nas\.lan/);
		act(() => connections.emitState('sftp://me@nas.lan', { kind: 'connected' }));
		await waitFor(() => expect(nas).toHaveAccessibleName(/NAS.*Connected/));
		expect(sidebarStatus()).toHaveTextContent('NAS: Connected');
		const disconnect = within(group).getByRole('button', { name: 'Disconnect NAS' });
		fireEvent.click(disconnect);
		await waitFor(() => expect(nas).toHaveAccessibleName(/NAS.*Not connected/));
		expect(connections.calls.some((call) => call.method === 'disconnect')).toBe(true);
	});

	it('deletes a saved connection from its menu after asking, starting on Cancel', async () => {
		const connections = new FakeConnectionsClient({
			connections: [draft({ host: 'nas.lan', name: 'NAS' })],
		});
		await renderWorkspace(undefined, undefined, undefined, { sidebar: true, connections });
		const nas = await within(await network()).findByRole('button', { name: /NAS/ });
		fireEvent.keyDown(nas, { key: 'ContextMenu' });
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Delete…' }));
		const confirm = await screen.findByRole('dialog', { name: 'Delete “NAS”?' });
		await waitFor(() =>
			expect(within(confirm).getByRole('button', { name: 'Cancel' })).toHaveFocus(),
		);
		fireEvent.click(within(confirm).getByRole('button', { name: 'Delete' }));
		await waitFor(() =>
			expect(
				within(screen.getByRole('group', { name: 'Network' })).queryByRole('button', {
					name: /^NAS/,
				}),
			).toBeNull(),
		);
	});

	it('forgets a recent server with its remembered password, and says when the keyring would not', async () => {
		const connections = new FakeConnectionsClient({
			recent: [{ key: 'sftp://pi', location: serverLocation('sftp://pi'), atMs: 1 }],
			keyring: 'locked',
		});
		await renderWorkspace(undefined, undefined, undefined, { sidebar: true, connections });
		const pi = await within(await network()).findByRole('button', { name: /sftp:\/\/pi/ });
		fireEvent.keyDown(pi, { key: 'ContextMenu' });
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Forget' }));
		await waitFor(() =>
			expect(sidebarStatus()).toHaveTextContent(/Forgot .*Its password could not be forgotten/),
		);
		expect(connections.calls).toContainEqual(expect.objectContaining({ method: 'forgetRecent' }));
	});

	it('opens the Connect dialog from Connect to Server…', async () => {
		await renderWorkspace(undefined, undefined, undefined, {
			sidebar: true,
			connections: new FakeConnectionsClient(),
		});
		fireEvent.click(
			await within(await network()).findByRole('button', { name: 'Connect to Server…' }),
		);
		expect(await screen.findByRole('dialog', { name: 'Connect to Server' })).toBeVisible();
	});

	it('is not shown without a connections service', async () => {
		await renderWorkspace(undefined, undefined, undefined, { sidebar: true });
		await screen.findByRole('group', { name: 'Places' });
		expect(screen.queryByRole('group', { name: 'Network' })).toBeNull();
	});
});

describe('a server tab', () => {
	async function openServerTab(error: VfsError | null, connections: FakeConnectionsClient) {
		const client = treeWithServer();
		if (error) client.failOpening(NAS, error);
		const tabs = new FakeTabsApi();
		await tabs.openTab(NAS);
		await renderWorkspace(client, tabs, undefined, { connections });
		return client;
	}

	it('says why it could not open and reconnects, then shows the folder', async () => {
		const offline: VfsError = { kind: 'unreachable', location: NAS, reason: 'refused' };
		const connections = new FakeConnectionsClient();
		const client = await openServerTab(offline, connections);
		const state = await screen.findByRole('heading', { name: 'Cannot reach me@nas.lan' });
		expect(state.parentElement).toHaveTextContent(/refused the connection/);
		client.succeedOpening(NAS);
		fireEvent.click(screen.getByRole('button', { name: 'Reconnect' }));
		expect(await screen.findByText('media')).toBeVisible();
	});

	it('asks to sign in and opens once the login is accepted', async () => {
		const needs: VfsError = {
			kind: 'authRequired',
			location: NAS,
			prompt: { kind: 'password', user: 'me' },
		};
		const connections = new FakeConnectionsClient({
			connect: (_key, answer) => (answer?.kind === 'password' ? null : needs),
		});
		const client = await openServerTab(needs, connections);
		fireEvent.click(await screen.findByRole('button', { name: 'Sign In…' }));
		const signIn = await screen.findByRole('dialog', { name: /^Sign in to/ });
		client.succeedOpening(NAS);
		fireEvent.change(within(signIn).getByLabelText('Password'), { target: { value: 'pw' } });
		fireEvent.click(within(signIn).getByRole('button', { name: 'Sign In' }));
		expect(await screen.findByText('media')).toBeVisible();
		expect(connections.calls.find((call) => call.method === 'connect')?.args[1]).toBe('password');
	});

	it('wears a server badge whose state is in its description', async () => {
		const connections = new FakeConnectionsClient();
		await openServerTab(null, connections);
		act(() => connections.emitState('sftp://me@nas.lan', { kind: 'connected' }));
		const tab = await screen.findByRole('tab', { selected: true });
		await waitFor(() => expect(tab).toHaveAccessibleDescription(/On a server: Connected/));
	});
});

describe('the path bar', () => {
	it('drops a password typed in a server address, says so, and goes on the next Enter', async () => {
		const client = treeWithServer();
		await renderWorkspace(client, undefined, undefined, {
			connections: new FakeConnectionsClient(),
		});
		fireEvent.keyDown(window, { key: 'l', ctrlKey: true });
		const input = await screen.findByRole('textbox', { name: /location/i });
		fireEvent.change(input, { target: { value: 'sftp://me:hunter2@nas.lan/' } });
		fireEvent.submit(input.closest('form')!);
		expect(await screen.findByText(/password was not kept/)).toBeVisible();
		expect((input as HTMLInputElement).value).toBe('sftp://me@nas.lan/');
		fireEvent.submit(input.closest('form')!);
		expect(await screen.findByText('media')).toBeVisible();
	});
});
