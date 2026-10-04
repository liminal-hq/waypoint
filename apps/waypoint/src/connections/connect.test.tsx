// Verifies the Connect dialog and the connection questions in a Main window against the fake connections service
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { stubLayout } from '../test/browseHarness';
import { renderWorkspace } from '../test/workspaceHarness';
import { connectStore, openConnectDialog } from './connectStore';
import {
	draft,
	FakeConnectionsClient,
	serverLocation,
	type ConnectScript,
} from './fakeConnectionsClient';

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

const at = serverLocation('sftp://nas.lan');

async function setup(client = new FakeConnectionsClient()) {
	const tabs = new FakeTabsApi();
	await renderWorkspace(undefined, tabs, undefined, { connections: client });
	act(() => openConnectDialog());
	const dialog = await screen.findByRole('dialog', { name: 'Connect to Server' });
	return { client, tabs, dialog };
}

const field = (dialog: HTMLElement, name: string | RegExp) =>
	within(dialog).getByRole('textbox', { name }) as HTMLInputElement;

describe('the Connect dialog', () => {
	it('starts on the address field, and an address fills the fields and drops its password', async () => {
		const { dialog } = await setup();
		const address = field(dialog, 'Address');
		await waitFor(() => expect(address).toHaveFocus());
		fireEvent.change(address, { target: { value: 'sftp://me:hunter2@NAS.lan:2222/srv' } });
		await waitFor(() => expect(field(dialog, 'Host').value).toBe('nas.lan'));
		expect(field(dialog, 'Port').value).toBe('2222');
		expect(field(dialog, 'User').value).toBe('me');
		expect(within(dialog).getByText(/password in the address was not kept/)).toBeVisible();
	});

	it('says why a test failed and keeps every field', async () => {
		const script: ConnectScript = () => ({ kind: 'unreachable', location: at, reason: 'refused' });
		const { dialog } = await setup(new FakeConnectionsClient({ connect: script }));
		fireEvent.change(field(dialog, 'Host'), { target: { value: 'nas.lan' } });
		fireEvent.change(field(dialog, 'User'), { target: { value: 'me' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		expect(
			await within(dialog).findByText('Not connected: the server refused the connection.'),
		).toBeVisible();
		expect(field(dialog, 'Host').value).toBe('nas.lan');
		expect(field(dialog, 'User').value).toBe('me');
	});

	it('puts a refused field’s message under it', async () => {
		const { dialog } = await setup();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
		const host = field(dialog, 'Host');
		await waitFor(() => expect(host).toHaveAttribute('aria-invalid', 'true'));
		expect(host).toHaveAccessibleDescription(/Type a host name or an IP address/);
	});

	it('saves, edits, duplicates and deletes connections', async () => {
		const client = new FakeConnectionsClient();
		const { dialog } = await setup(client);
		fireEvent.change(field(dialog, 'Host'), { target: { value: 'nas.lan' } });
		fireEvent.change(field(dialog, /^Name/), { target: { value: 'NAS' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
		await within(dialog).findByRole('button', { name: 'Edit NAS' });
		expect(screen.getByRole('dialog', { name: 'Edit Connection' })).toBeVisible();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Duplicate NAS' }));
		await within(dialog).findByRole('button', { name: 'Edit NAS (copy)' });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Delete NAS (copy)' }));
		const confirm = await screen.findByRole('dialog', { name: 'Delete “NAS (copy)”?' });
		await waitFor(() =>
			expect(within(confirm).getByRole('button', { name: 'Cancel' })).toHaveFocus(),
		);
		fireEvent.click(within(confirm).getByRole('button', { name: 'Delete' }));
		await waitFor(() =>
			expect(within(dialog).queryByRole('button', { name: 'Edit NAS (copy)' })).toBeNull(),
		);
		expect(client.calls.find((call) => call.method === 'remove')?.args).toEqual(['c2', true]);
	});

	it('asks to trust an unknown host key, starting on Cancel, then connects and opens a tab', async () => {
		let asked = false;
		const script: ConnectScript = (_key, answer) => {
			if (!answer && !asked) {
				asked = true;
				return {
					kind: 'hostKeyUnknown',
					location: at,
					key: { host: 'nas.lan', algorithm: 'ssh-ed25519', fingerprint: 'SHA256:abc' },
				};
			}
			return null;
		};
		const { dialog, tabs, client } = await setup(new FakeConnectionsClient({ connect: script }));
		fireEvent.change(field(dialog, 'Host'), { target: { value: 'nas.lan' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Connect' }));
		const trust = await screen.findByRole('dialog', { name: 'Trust nas.lan?' });
		expect(within(trust).getByText('SHA256:abc')).toBeVisible();
		await waitFor(() =>
			expect(within(trust).getByRole('button', { name: 'Cancel' })).toHaveFocus(),
		);
		fireEvent.click(within(trust).getByRole('button', { name: 'Trust and Remember' }));
		await waitFor(() =>
			expect(screen.queryByRole('dialog', { name: 'Connect to Server' })).toBeNull(),
		);
		const snapshot = await tabs.getSnapshot();
		expect(snapshot.tabs.some((tab) => tab.location.uri === 'sftp://nas.lan/')).toBe(true);
		expect(
			client.calls.filter((call) => call.method === 'test').map((call) => call.args[1]),
		).toEqual([null, 'trustHostKey']);
	});

	it('warns about a changed host key and trusts it only through its own action', async () => {
		const changed: VfsError = {
			kind: 'hostKeyChanged',
			location: at,
			change: {
				host: 'nas.lan',
				recordedAlgorithm: 'ssh-ed25519',
				recordedFingerprint: 'SHA256:old',
				offeredAlgorithm: 'ssh-ed25519',
				offeredFingerprint: 'SHA256:new',
			},
		};
		const script: ConnectScript = (_key, answer) =>
			answer?.kind === 'trustChangedHostKey' ? null : changed;
		const { dialog } = await setup(new FakeConnectionsClient({ connect: script }));
		fireEvent.change(field(dialog, 'Host'), { target: { value: 'nas.lan' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		let warning = await screen.findByRole('dialog', { name: 'The key of nas.lan has changed' });
		expect(within(warning).getByText('SHA256:old')).toBeVisible();
		expect(within(warning).getByText('SHA256:new')).toBeVisible();
		expect(within(warning).getByRole('alert')).toHaveTextContent(/pretending/);
		await waitFor(() =>
			expect(within(warning).getByRole('button', { name: 'Cancel' })).toHaveFocus(),
		);
		fireEvent.click(within(warning).getByRole('button', { name: 'Cancel' }));
		expect(await within(dialog).findByText('Not connected: cancelled.')).toBeVisible();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		warning = await screen.findByRole('dialog', { name: 'The key of nas.lan has changed' });
		fireEvent.click(within(warning).getByRole('button', { name: 'Trust the New Key' }));
		expect(await within(dialog).findByText(/^Connected to sftp:\/\/nas.lan\//)).toBeVisible();
	});

	it('asks for a password with Remember, or says why it cannot remember', async () => {
		const script: ConnectScript = (_key, answer) =>
			answer?.kind === 'password'
				? null
				: { kind: 'authRequired', location: at, prompt: { kind: 'password', user: 'me' } };
		const client = new FakeConnectionsClient({ connect: script });
		const { dialog } = await setup(client);
		fireEvent.change(field(dialog, 'Host'), { target: { value: 'nas.lan' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		const signIn = await screen.findByRole('dialog', { name: /^Sign in to/ });
		const secret = within(signIn).getByLabelText('Password');
		await waitFor(() => expect(secret).toHaveFocus());
		fireEvent.change(secret, { target: { value: 'hunter2' } });
		fireEvent.click(await within(signIn).findByRole('checkbox', { name: 'Remember in keyring' }));
		fireEvent.click(within(signIn).getByRole('button', { name: 'Sign In' }));
		expect(await within(dialog).findByText(/remembered in the keyring/)).toBeVisible();
		expect(
			client.calls
				.filter((call) => call.method === 'test')
				.at(-1)
				?.args.slice(1),
		).toEqual(['password', true]);

		client.keyring = 'locked';
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		const again = await screen.findByRole('dialog', { name: /^Sign in to/ });
		expect(await within(again).findByText(/the keyring is locked/)).toBeVisible();
		expect(within(again).queryByRole('checkbox')).toBeNull();
	});

	it('opens a saved connection for editing', async () => {
		const client = new FakeConnectionsClient({
			connections: [draft({ host: 'nas.lan', user: 'me', name: 'NAS' })],
		});
		const tabs = new FakeTabsApi();
		await renderWorkspace(undefined, tabs, undefined, { connections: client });
		act(() => openConnectDialog({ mode: 'edit', id: 'c1' }));
		const dialog = await screen.findByRole('dialog', { name: 'Edit Connection' });
		await waitFor(() => expect(field(dialog, 'Host').value).toBe('nas.lan'));
		expect(within(dialog).getByRole('button', { name: 'Edit NAS' })).toHaveAttribute(
			'aria-current',
			'true',
		);
	});
});
