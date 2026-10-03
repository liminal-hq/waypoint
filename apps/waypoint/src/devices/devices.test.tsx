// Verifies the Devices section in the sidebar against the fake volumes service: listing, actions, live changes, unlock and announcements
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { stubLayout } from '../test/browseHarness';
import { renderWorkspace } from '../test/workspaceHarness';
import { FakeDevicesClient, fakeStatus, fakeVolume } from './fakeDevicesClient';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

const SYSTEM = fakeVolume('root', {
	label: 'System',
	kind: 'internal',
	isSystem: true,
	mountPoint: '/',
	canEject: false,
	canUnmount: false,
});
const STICK = fakeVolume('stick', {
	label: 'Backup',
	total: 500_000_000_000,
	free: 125_000_000_000,
});

async function setup(client: FakeDevicesClient) {
	const h = await renderWorkspace(undefined, undefined, undefined, {
		sidebar: true,
		devices: client,
	});
	await screen.findByRole('button', { name: 'Home' });
	return h;
}

const devices = () => screen.findByRole('group', { name: 'Devices' });
const status = () =>
	within(screen.getByRole('navigation', { name: 'Sidebar' })).getByRole('status');

describe('listing', () => {
	it('shows each volume with its space written out', async () => {
		await setup(new FakeDevicesClient([SYSTEM, STICK]));
		const group = await devices();
		expect(within(group).getByRole('button', { name: /Backup.*125.*free of.*500/ })).toBeVisible();
		expect(within(group).getByRole('button', { name: /System/ })).toBeVisible();
	});

	it('says a nearly full volume is nearly full', async () => {
		await setup(
			new FakeDevicesClient([fakeVolume('a', { label: 'Tight', total: 1000, free: 20 })]),
		);
		expect(
			within(await devices()).getByRole('button', { name: /Tight.*Almost full/ }),
		).toBeVisible();
	});

	it('puts the warning in an element of its own and the full text on the tooltip', async () => {
		await setup(
			new FakeDevicesClient([fakeVolume('a', { label: 'Tight', total: 1000, free: 20 })]),
		);
		const row = within(await devices()).getByRole('button', { name: /Tight.*Almost full/ });
		const warning = within(row).getByText('Almost full');
		expect(warning).not.toBe(row);
		expect(warning.className).toContain('warning');
		expect(row.getAttribute('title')).toMatch(/Tight\. .* free of .*\. Almost full/);
	});

	it('lets the caption and the warning wrap instead of cutting them off', () => {
		// jsdom lays nothing out, so the stylesheet is checked as written.
		const css = readFileSync(resolve(process.cwd(), 'src/devices/DeviceList.module.css'), 'utf8');
		const rule = /\.detail,\s*\.warning\s*\{([^}]*)\}/.exec(css)?.[1] ?? '';
		expect(rule).toMatch(/white-space:\s*normal/);
		expect(rule).toMatch(/overflow-wrap:\s*anywhere/);
		expect(rule).not.toMatch(/nowrap|text-overflow/);
	});

	it('has no section without a client or when the plugin cannot list', async () => {
		await renderWorkspace(undefined, undefined, undefined, { sidebar: true });
		await screen.findByRole('button', { name: 'Home' });
		expect(screen.queryByRole('group', { name: 'Devices' })).toBeNull();
		cleanup();
		await setup(new FakeDevicesClient([STICK], fakeStatus(['list'])));
		await waitFor(() => expect(screen.queryByRole('group', { name: 'Devices' })).toBeNull());
	});
});

describe('actions', () => {
	it('offers unmount and eject, and hides the ones the plugin says are unavailable', async () => {
		await setup(new FakeDevicesClient([STICK], fakeStatus(['eject'])));
		const group = await devices();
		expect(within(group).getByRole('button', { name: 'Unmount Backup' })).toBeVisible();
		expect(within(group).queryByRole('button', { name: 'Eject Backup' })).toBeNull();
	});

	it('unmounts and announces it', async () => {
		const client = new FakeDevicesClient([STICK]);
		await setup(client);
		fireEvent.click(within(await devices()).getByRole('button', { name: 'Unmount Backup' }));
		await waitFor(() => expect(status()).toHaveTextContent('Unmounted Backup'));
		expect(client.calls).toEqual([{ action: 'unmount', id: 'stick' }]);
		expect(within(await devices()).getByRole('button', { name: 'Mount Backup' })).toBeVisible();
	});

	it('ejects, announces it once and drops the row', async () => {
		const client = new FakeDevicesClient([STICK]);
		await setup(client);
		fireEvent.click(within(await devices()).getByRole('button', { name: 'Eject Backup' }));
		await waitFor(() =>
			expect(status()).toHaveTextContent('Ejected Backup. It is safe to remove.'),
		);
		await waitFor(() => expect(screen.queryByRole('button', { name: /Backup/ })).toBeNull());
		expect(status()).toHaveTextContent('Ejected Backup');
	});

	it('mounts a volume that is not mounted when its row is activated', async () => {
		const client = new FakeDevicesClient([
			fakeVolume('disc', { label: 'Disc', mountPoint: null, canMount: true, free: null }),
		]);
		await setup(client);
		fireEvent.click(within(await devices()).getByRole('button', { name: /^Disc/ }));
		await waitFor(() => expect(client.calls).toEqual([{ action: 'mount', id: 'disc' }]));
		await waitFor(() => expect(status()).toHaveTextContent('Mounted Disc'));
	});

	it('says what is using a busy volume and reports it', async () => {
		const client = new FakeDevicesClient([STICK]);
		await setup(client);
		client.failNext({ kind: 'busy', by: 'bash' });
		fireEvent.click(within(await devices()).getByRole('button', { name: 'Unmount Backup' }));
		await waitFor(() =>
			expect(status()).toHaveTextContent('Could not unmount Backup because bash is using it.'),
		);
	});

	it('disables a row while its action runs', async () => {
		const client = new FakeDevicesClient([STICK]);
		await setup(client);
		client.hold();
		const unmount = within(await devices()).getByRole('button', { name: 'Unmount Backup' });
		fireEvent.click(unmount);
		await waitFor(() => expect(unmount).toBeDisabled());
		await act(async () => client.release());
		await waitFor(() => expect(client.calls).toHaveLength(1));
	});
});

describe('live changes', () => {
	it('adds and removes rows and announces them', async () => {
		const client = new FakeDevicesClient([SYSTEM]);
		await setup(client);
		await devices();
		act(() => client.plug(fakeVolume('card', { label: 'Camera' })));
		expect(await within(await devices()).findByRole('button', { name: /^Camera/ })).toBeVisible();
		expect(status()).toHaveTextContent('Camera connected');
		act(() => client.unplug('card'));
		await waitFor(() =>
			expect(
				within(screen.getByRole('group', { name: 'Devices' })).queryByText('Camera'),
			).toBeNull(),
		);
		expect(status()).toHaveTextContent('Camera removed');
	});

	it('ignores an event that is not newer than the last one seen', async () => {
		const client = new FakeDevicesClient([SYSTEM, STICK]);
		await setup(client);
		await devices();
		act(() => client.change('stick', { label: 'Renamed' }));
		await within(await devices()).findByRole('button', { name: /^Renamed/ });
		act(() => client.sendStale());
		expect(within(await devices()).getByRole('button', { name: /^Renamed/ })).toBeVisible();
	});
});

describe('unlock', () => {
	function locked() {
		const client = new FakeDevicesClient([
			fakeVolume('vault', { label: 'Vault', kind: 'encrypted', mountPoint: null, locked: true }),
		]);
		client.passphrases.set('vault', 'correct horse');
		return client;
	}

	it('asks for a passphrase with no way to remember it while remembering is off', async () => {
		await setup(locked());
		fireEvent.click(within(await devices()).getByRole('button', { name: 'Unlock Vault' }));
		const dialog = await screen.findByRole('dialog', { name: 'Unlock Vault' });
		const field = within(dialog).getByLabelText('Passphrase');
		expect(field).toHaveAttribute('type', 'password');
		expect(within(dialog).queryByRole('checkbox')).toBeNull();
	});

	it('stays open on a wrong passphrase and says so', async () => {
		const client = locked();
		await setup(client);
		fireEvent.click(within(await devices()).getByRole('button', { name: 'Unlock Vault' }));
		const dialog = await screen.findByRole('dialog', { name: 'Unlock Vault' });
		fireEvent.change(within(dialog).getByLabelText('Passphrase'), { target: { value: 'nope' } });
		fireEvent.keyDown(within(dialog).getByLabelText('Passphrase'), { key: 'Enter' });
		expect(await within(dialog).findByText(/did not unlock/)).toBeVisible();
		expect(screen.getByRole('dialog', { name: 'Unlock Vault' })).toBeVisible();
	});

	it('unlocks, mounts what appears and closes', async () => {
		const client = locked();
		await setup(client);
		fireEvent.click(within(await devices()).getByRole('button', { name: 'Unlock Vault' }));
		const dialog = await screen.findByRole('dialog', { name: 'Unlock Vault' });
		fireEvent.change(within(dialog).getByLabelText('Passphrase'), {
			target: { value: 'correct horse' },
		});
		fireEvent.click(within(dialog).getByRole('button', { name: 'Unlock' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(client.calls.map((call) => call.action)).toEqual(['unlock', 'mount']);
		expect(client.calls[1]?.id).toBe('vault-open');
	});
});

describe('remembering a passphrase (D153)', () => {
	function lockedWith(status: ReturnType<typeof fakeStatus>, overrides = {}) {
		const client = new FakeDevicesClient(
			[
				fakeVolume('vault', {
					label: 'Vault',
					kind: 'encrypted',
					mountPoint: null,
					locked: true,
					uuid: 'luks-1',
					...overrides,
				}),
			],
			status,
		);
		client.passphrases.set('vault', 'correct horse');
		return client;
	}

	async function openDialog() {
		fireEvent.click(within(await devices()).getByRole('button', { name: 'Unlock Vault' }));
		return screen.findByRole('dialog', { name: 'Unlock Vault' });
	}

	async function unlockWith(dialog: HTMLElement) {
		fireEvent.change(within(dialog).getByLabelText('Passphrase'), {
			target: { value: 'correct horse' },
		});
		fireEvent.click(within(dialog).getByRole('button', { name: 'Unlock' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
	}

	it('offers "Remember in keyring" unticked when the keyring works and the switch is on', async () => {
		await setup(lockedWith(fakeStatus([], 'on')));
		const dialog = await openDialog();
		const box = within(dialog).getByRole('checkbox', { name: 'Remember in keyring' });
		expect(box).not.toBeChecked();
		expect(box).toHaveAccessibleDescription(/unlocks by itself/);
	});

	it('sends the choice with the unlock and says the passphrase was kept', async () => {
		const client = lockedWith(fakeStatus([], 'on'));
		await setup(client);
		const dialog = await openDialog();
		fireEvent.click(within(dialog).getByRole('checkbox', { name: 'Remember in keyring' }));
		await unlockWith(dialog);
		expect(client.calls[0]).toMatchObject({ action: 'unlock', remember: true });
		expect(status()).toHaveTextContent('Saved the passphrase of Vault in the keyring');
	});

	it('does not remember when the box is left unticked', async () => {
		const client = lockedWith(fakeStatus([], 'on'));
		await setup(client);
		await unlockWith(await openDialog());
		expect(client.calls[0]).toMatchObject({ action: 'unlock', remember: false });
	});

	it('hides the option where the switch is off, and the dialog says nothing of it', async () => {
		await setup(lockedWith(fakeStatus([], 'disabled')));
		const dialog = await openDialog();
		expect(within(dialog).queryByRole('checkbox')).toBeNull();
		expect(within(dialog).queryByText(/cannot be remembered/)).toBeNull();
	});

	it('says why it cannot be remembered when it is on but there is no keyring', async () => {
		await setup(lockedWith(fakeStatus([], 'no-keyring')));
		const dialog = await openDialog();
		expect(within(dialog).queryByRole('checkbox')).toBeNull();
		expect(
			within(dialog).getByText('The passphrase cannot be remembered: no keyring is running.'),
		).toBeVisible();
	});

	it('says why it cannot be remembered when the keyring is locked', async () => {
		await setup(lockedWith(fakeStatus([], 'keyring-locked')));
		const dialog = await openDialog();
		expect(
			within(dialog).getByText('The passphrase cannot be remembered: the keyring is locked.'),
		).toBeVisible();
	});

	it('tells the person when the volume unlocked but the passphrase could not be kept', async () => {
		const client = lockedWith(fakeStatus([], 'on'));
		client.rememberOutcome = {
			state: 'failed',
			reason: 'keyring-locked',
			message: 'The keyring is locked.',
		};
		await setup(client);
		const dialog = await openDialog();
		fireEvent.click(within(dialog).getByRole('checkbox', { name: 'Remember in keyring' }));
		await unlockWith(dialog);
		expect(client.calls.map((call) => call.action)).toEqual(['unlock', 'mount']);
		expect(status()).toHaveTextContent(
			'Unlocked Vault, but its passphrase was not remembered: the keyring is locked.',
		);
	});

	it('forgets a remembered passphrase from the volume row, and only offers it for one', async () => {
		const client = new FakeDevicesClient(
			[
				fakeVolume('vault', { label: 'Vault', uuid: 'luks-1', remembered: true }),
				fakeVolume('plain', { label: 'Plain' }),
			],
			fakeStatus([], 'on'),
		);
		await setup(client);
		const group = await devices();
		expect(
			within(group).queryByRole('button', { name: 'Forget the saved passphrase of Plain' }),
		).toBeNull();
		fireEvent.click(
			within(group).getByRole('button', { name: 'Forget the saved passphrase of Vault' }),
		);
		await waitFor(() =>
			expect(
				within(group).queryByRole('button', { name: 'Forget the saved passphrase of Vault' }),
			).toBeNull(),
		);
		expect(client.calls).toEqual([{ action: 'forget', id: 'vault' }]);
		expect(status()).toHaveTextContent('Forgot the saved passphrase of Vault');
	});

	it('does not offer to forget while remembering is off', async () => {
		await setup(
			new FakeDevicesClient(
				[fakeVolume('vault', { label: 'Vault', uuid: 'luks-1', remembered: true })],
				fakeStatus([], 'disabled'),
			),
		);
		expect(
			within(await devices()).queryByRole('button', { name: /Forget the saved passphrase/ }),
		).toBeNull();
	});
});

describe('keyboard', () => {
	it('moves between rows with the arrow keys', async () => {
		await setup(new FakeDevicesClient([SYSTEM, STICK]));
		const group = await devices();
		const first = within(group).getByRole('button', { name: /System/ });
		first.focus();
		fireEvent.keyDown(first, { key: 'ArrowDown' });
		expect(within(group).getByRole('button', { name: /Backup.*free of/ })).toHaveFocus();
	});
});
