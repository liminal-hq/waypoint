// Verifies the Connect dialog's form for each protocol: SMB's domain and share, WebDAV's sign-in, Nextcloud and certificate trust, and what SFTP alone asks for
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
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

/** A build with every protocol on, as the app has once the switches are turned on. */
const everything = (options: ConstructorParameters<typeof FakeConnectionsClient>[0] = {}) =>
	new FakeConnectionsClient({ schemes: ['sftp', 'smb', 'davs', 'dav'], ...options });

async function setup(client: FakeConnectionsClient, protocol?: string) {
	await renderWorkspace(undefined, new FakeTabsApi(), undefined, { connections: client });
	act(() => openConnectDialog());
	const dialog = await screen.findByRole('dialog', { name: 'Connect to Server' });
	if (protocol) {
		await waitFor(() =>
			expect(within(dialog).getByRole('combobox', { name: 'Protocol' })).toHaveTextContent(/SMB/),
		);
		fireEvent.change(within(dialog).getByRole('combobox', { name: 'Protocol' }), {
			target: { value: protocol },
		});
	}
	return dialog;
}

const box = (dialog: HTMLElement, name: string | RegExp) =>
	within(dialog).getByRole('textbox', { name }) as HTMLInputElement;
const maybe = (dialog: HTMLElement, name: string | RegExp) =>
	within(dialog).queryByRole('textbox', { name });
const methods = (dialog: HTMLElement) =>
	within(within(dialog).getByRole('group', { name: 'Sign in with' }))
		.getAllByRole('radio')
		.map((radio) => radio.parentElement?.textContent);
const lastCall = (client: FakeConnectionsClient, method: string) =>
	client.calls.filter((call) => call.method === method).at(-1);

describe('an SMB connection', () => {
	it('asks for a domain and a share, and not for what only SSH has', async () => {
		const dialog = await setup(everything(), 'smb');
		expect(box(dialog, 'Domain')).toBeVisible();
		expect(box(dialog, /^Share/)).toBeVisible();
		expect(methods(dialog)).toEqual(['Automatically', 'Password']);
		expect(maybe(dialog, 'Jump host')).toBeNull();
		expect(maybe(dialog, 'Start folder')).toBeNull();
		expect(box(dialog, 'Address').placeholder).toBe('smb://domain;user@host/share');
	});

	it('saves the domain with the user as domain;user and the share as the start folder', async () => {
		const client = everything();
		const dialog = await setup(client, 'smb');
		fireEvent.change(box(dialog, 'Host'), { target: { value: 'files.lan' } });
		fireEvent.change(box(dialog, 'Domain'), { target: { value: 'WORK' } });
		fireEvent.change(box(dialog, 'User'), { target: { value: 'me' } });
		fireEvent.change(box(dialog, /^Share/), { target: { value: 'Projects' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
		await within(dialog).findByRole('button', { name: /Edit/ });
		const saved = lastCall(client, 'add')?.args[0] as ReturnType<typeof draft>;
		expect(saved).toMatchObject({
			scheme: 'smb',
			host: 'files.lan',
			user: 'WORK;me',
			startFolder: '/Projects',
		});
	});

	it('reads an address with a domain and a share into the fields', async () => {
		const dialog = await setup(everything(), 'smb');
		fireEvent.change(box(dialog, 'Address'), {
			target: { value: 'smb://WORK;me@files.lan/Projects/2026' },
		});
		await waitFor(() => expect(box(dialog, 'Host').value).toBe('files.lan'));
		expect(box(dialog, 'Domain').value).toBe('WORK');
		expect(box(dialog, 'User').value).toBe('me');
		expect(box(dialog, /^Share/).value).toBe('Projects/2026');
	});

	it('keeps the share empty for the share browser', async () => {
		const client = everything();
		const dialog = await setup(client, 'smb');
		fireEvent.change(box(dialog, 'Host'), { target: { value: 'files.lan' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
		await within(dialog).findByRole('button', { name: /Edit/ });
		expect((lastCall(client, 'add')?.args[0] as ReturnType<typeof draft>).startFolder).toBeNull();
		expect(within(dialog).getByText(/Leave it empty to browse the server’s shares/)).toBeVisible();
	});

	it('refuses a domain with no user before anything is sent', async () => {
		const client = everything();
		const dialog = await setup(client, 'smb');
		fireEvent.change(box(dialog, 'Host'), { target: { value: 'files.lan' } });
		fireEvent.change(box(dialog, 'Domain'), { target: { value: 'WORK' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		const user = box(dialog, 'User');
		await waitFor(() => expect(user).toHaveAttribute('aria-invalid', 'true'));
		expect(user).toHaveAccessibleDescription(/Give the user name that goes with the domain/);
		expect(lastCall(client, 'test')).toBeUndefined();
	});

	it('sends the password with the domain in the user it signs in as', async () => {
		const client = everything();
		const dialog = await setup(client, 'smb');
		fireEvent.change(box(dialog, 'Host'), { target: { value: 'files.lan' } });
		fireEvent.change(box(dialog, 'Domain'), { target: { value: 'WORK' } });
		fireEvent.change(box(dialog, 'User'), { target: { value: 'me' } });
		fireEvent.click(within(dialog).getByRole('radio', { name: 'Password' }));
		fireEvent.change(
			within(dialog).getByLabelText('Password', { selector: 'input[type="password"]' }),
			{
				target: { value: 'pw' },
			},
		);
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		await within(dialog).findByText(/^Connected to/);
		expect(lastCall(client, 'test')?.args.slice(1)).toEqual(['password', false]);
	});

	it('shows a saved SMB connection’s domain and share when it is edited', async () => {
		const client = everything({
			connections: [
				draft({ scheme: 'smb', host: 'files.lan', user: 'WORK;me', startFolder: '/Projects' }),
			],
		});
		await renderWorkspace(undefined, new FakeTabsApi(), undefined, { connections: client });
		act(() => openConnectDialog({ mode: 'edit', id: 'c1' }));
		const dialog = await screen.findByRole('dialog', { name: 'Edit Connection' });
		await waitFor(() => expect(box(dialog, 'Domain').value).toBe('WORK'));
		expect(box(dialog, 'User').value).toBe('me');
		expect(box(dialog, /^Share/).value).toBe('Projects');
	});
});

describe('a WebDAV connection', () => {
	it('offers a password or an access token, and no key file or jump host', async () => {
		const dialog = await setup(everything(), 'davs');
		expect(methods(dialog)).toEqual(['Automatically', 'Password', 'Access token']);
		expect(maybe(dialog, 'Jump host')).toBeNull();
		expect(maybe(dialog, 'Domain')).toBeNull();
		expect(box(dialog, 'Address').placeholder).toBe('davs://host/folder');
	});

	it('warns that plain HTTP is not encrypted, and only then', async () => {
		const dialog = await setup(everything(), 'dav');
		expect(within(dialog).getByRole('note', { name: '' })).toHaveTextContent(/not encrypted/);
		fireEvent.change(within(dialog).getByRole('combobox', { name: 'Protocol' }), {
			target: { value: 'davs' },
		});
		expect(within(dialog).queryByText(/This connection is not encrypted/)).toBeNull();
	});

	it('chooses how a password is sent, and saves it as an option of the connection', async () => {
		const client = everything();
		const dialog = await setup(client, 'davs');
		fireEvent.change(box(dialog, 'Host'), { target: { value: 'dav.example.com' } });
		fireEvent.click(within(dialog).getByRole('radio', { name: 'Password' }));
		fireEvent.change(within(dialog).getByRole('combobox', { name: 'Password sent as' }), {
			target: { value: 'basic' },
		});
		fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
		await within(dialog).findByRole('button', { name: /Edit/ });
		const saved = lastCall(client, 'add')?.args[0] as ReturnType<typeof draft>;
		expect(saved.auth).toBe('password');
		expect(saved.options.davAuth).toBe('basic');
		expect(saved.options.davPreset).toBeNull();
	});

	it('sends an access token as the secret it is', async () => {
		const client = everything();
		const dialog = await setup(client, 'davs');
		fireEvent.change(box(dialog, 'Host'), { target: { value: 'dav.example.com' } });
		fireEvent.click(within(dialog).getByRole('radio', { name: 'Access token' }));
		fireEvent.change(
			within(dialog).getByLabelText('Access token', { selector: 'input[type="password"]' }),
			{
				target: { value: 'tok' },
			},
		);
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		await within(dialog).findByText(/^Connected to/);
		expect(lastCall(client, 'test')?.args.slice(1)).toEqual(['passphrase', false]);
	});

	it('fills in a Nextcloud files folder through Rust and saves the dialect', async () => {
		const client = everything();
		const dialog = await setup(client, 'davs');
		fireEvent.change(box(dialog, 'Host'), { target: { value: 'cloud.example.com' } });
		fireEvent.change(box(dialog, 'User'), { target: { value: 'alice' } });
		fireEvent.click(within(dialog).getByRole('checkbox', { name: 'This is a Nextcloud server' }));
		fireEvent.click(within(dialog).getByRole('button', { name: 'Fill in my files folder' }));
		await waitFor(() =>
			expect(box(dialog, 'Address').value).toBe(
				'davs://alice@cloud.example.com/remote.php/dav/files/alice',
			),
		);
		expect(lastCall(client, 'nextcloudAddress')?.args).toEqual(['cloud.example.com', 'alice']);
		await waitFor(() => expect(box(dialog, 'Host').value).toBe('cloud.example.com'));
		fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));
		await within(dialog).findByRole('button', { name: /Edit/ });
		const saved = lastCall(client, 'add')?.args[0] as ReturnType<typeof draft>;
		expect(saved.startFolder).toBe('/remote.php/dav/files/alice');
		expect(saved.options.davPreset).toBe('nextcloud');
	});

	it('needs a host and a user before it can fill in the Nextcloud folder', async () => {
		const dialog = await setup(everything(), 'davs');
		fireEvent.click(within(dialog).getByRole('checkbox', { name: 'This is a Nextcloud server' }));
		expect(within(dialog).getByRole('button', { name: 'Fill in my files folder' })).toBeDisabled();
	});

	it('asks to trust an untrusted certificate, starting on Cancel, and trusts it for the connection', async () => {
		const location = serverLocation('davs://alice@cloud.example.com');
		const script: ConnectScript = (_key, answer) =>
			answer?.kind === 'trustCertificate'
				? null
				: {
						kind: 'certificateUntrusted',
						location,
						certificate: {
							subject: 'cloud.example.com',
							issuer: 'cloud.example.com',
							fingerprint: 'AA:BB',
							reason: 'it is self-signed',
						},
					};
		const client = everything({ connect: script });
		const dialog = await setup(client, 'davs');
		fireEvent.change(box(dialog, 'Host'), { target: { value: 'cloud.example.com' } });
		fireEvent.change(box(dialog, 'User'), { target: { value: 'alice' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Test' }));
		const trust = await screen.findByRole('dialog', {
			name: /Trust the certificate of/,
		});
		expect(within(trust).getByText('it is self-signed')).toBeVisible();
		expect(within(trust).getByText('AA:BB')).toBeVisible();
		await waitFor(() =>
			expect(within(trust).getByRole('button', { name: 'Cancel' })).toHaveFocus(),
		);
		fireEvent.click(within(trust).getByRole('button', { name: 'Trust for This Connection' }));
		expect(await within(dialog).findByText(/^Connected to/)).toBeVisible();
		expect(
			client.calls.filter((call) => call.method === 'test').map((call) => call.args[1]),
		).toEqual([null, 'trustCertificate']);
	});
});

describe('an SFTP connection', () => {
	it('keeps its key file, jump host and start folder, and none of the others’ fields', async () => {
		const dialog = await setup(everything());
		expect(methods(dialog)).toEqual(['Automatically', 'Password', 'Key file']);
		expect(maybe(dialog, 'Domain')).toBeNull();
		expect(maybe(dialog, /^Share/)).toBeNull();
		fireEvent.click(within(dialog).getByText('More options'));
		expect(box(dialog, 'Jump host')).toBeInTheDocument();
		expect(box(dialog, 'Start folder')).toBeInTheDocument();
	});

	it('offers More options as a disclosure button with its state', async () => {
		const dialog = await setup(everything());
		const toggle = within(dialog).getByRole('button', { name: 'More options' });
		expect(toggle).toHaveAttribute('aria-expanded', 'false');
		expect(maybe(dialog, 'Jump host')).toBeNull();
		fireEvent.click(toggle);
		expect(toggle).toHaveAttribute('aria-expanded', 'true');
		const panel = document.getElementById(toggle.getAttribute('aria-controls')!)!;
		expect(panel).not.toHaveAttribute('hidden');
		expect(within(panel).getByRole('textbox', { name: 'Jump host' })).toBeInTheDocument();
		fireEvent.click(toggle);
		expect(toggle).toHaveAttribute('aria-expanded', 'false');
		expect(panel).toHaveAttribute('hidden');
	});

	it('drops a way to sign in the protocol it changes to does not have', async () => {
		const dialog = await setup(everything());
		fireEvent.click(within(dialog).getByRole('radio', { name: 'Key file' }));
		fireEvent.change(within(dialog).getByRole('combobox', { name: 'Protocol' }), {
			target: { value: 'smb' },
		});
		expect(within(dialog).getByRole('radio', { name: 'Automatically' })).toBeChecked();
		expect(within(dialog).queryByRole('radio', { name: 'Key file' })).toBeNull();
	});
});
