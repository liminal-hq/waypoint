// Verifies Overview over a whole workspace: its stats, a card for each volume with its states, the Trash's card, the keyboard, and the sidebar and palette entries
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dismissNotice } from '../app/notices';
import { COMMANDS } from '../commands/registry';
import { FakeDevicesClient, fakeStatus, fakeVolume } from '../devices/fakeDevicesClient';
import { t } from '../i18n/messages';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { stubLayout } from '../test/browseHarness';
import { HOME, createTree, renderWorkspace } from '../test/workspaceHarness';
import { FakeTrashClient } from '../trash/fakeTrashClient';
import { OVERVIEW_LOCATION } from './overviewLocation';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
	vi.restoreAllMocks();
});

const SYSTEM = fakeVolume('root', {
	label: 'System',
	kind: 'internal',
	isSystem: true,
	mountPoint: '/',
	fileSystem: 'ext4',
	total: 500_000_000_000,
	free: 200_000_000_000,
	canEject: false,
	canUnmount: false,
});
const BACKUP = fakeVolume('stick', {
	label: 'Backup',
	mountPoint: '/run/media/test/Backup',
	fileSystem: 'exfat',
	total: 128_000_000_000,
	free: 100_000_000_000,
});
const NAS = fakeVolume('nas', {
	label: 'Media share',
	kind: 'network',
	isSystem: false,
	mountPoint: '/run/user/1000/gvfs/media',
	fileSystem: 'cifs',
	total: null,
	free: null,
	canEject: false,
});
const VAULT = fakeVolume('vault', {
	label: 'Vault',
	kind: 'encrypted',
	mountPoint: null,
	fileSystem: 'crypto_LUKS',
	locked: true,
	canMount: false,
	canUnmount: false,
});

interface Options {
	volumes?: ReturnType<typeof fakeVolume>[];
	devices?: FakeDevicesClient | null;
	trash?: FakeTrashClient | null;
}

async function setup({ volumes = [SYSTEM, BACKUP], devices, trash }: Options = {}) {
	const client = createTree();
	const tabs = new FakeTabsApi();
	await tabs.openTab(OVERVIEW_LOCATION);
	const volumesClient = devices === undefined ? new FakeDevicesClient(volumes) : devices;
	const trashClient = trash === undefined ? new FakeTrashClient({ count: 0 }) : trash;
	const h = await renderWorkspace(
		client,
		tabs,
		new FakePlacesClient({ places: fakePlaces('/home/test') }),
		{
			sidebar: true,
			...(volumesClient ? { devices: volumesClient } : {}),
			...(trashClient ? { trash: trashClient, ops: trashClient.opsClient() } : {}),
		},
	);
	return { ...h, devices: volumesClient, trash: trashClient };
}

const page = () => screen.findByRole('region', { name: 'Overview' });
const card = async (name: string) => within(await page()).findByRole('group', { name });
const stat = async (id: string) => {
	const tile = (await page()).querySelector(`[data-stat="${id}"]`);
	if (!tile) throw new Error(`no ${id} stat`);
	return within(tile as HTMLElement);
};

describe('the page', () => {
	it('opens as the content of the tab with no listing of its own', async () => {
		const h = await setup();
		expect(await page()).toBeInTheDocument();
		expect(screen.queryByRole('listbox', { name: 'Files' })).not.toBeInTheDocument();
		// Nothing was asked of the file system for a folder that is not one.
		expect(h.client.openCount).toBe(0);
	});

	it('shows the four headline stats, saying what Capacity counts', async () => {
		await setup({ volumes: [SYSTEM, BACKUP, NAS] });
		const capacity = await stat('capacity');
		await waitFor(() => expect(capacity.getByText('628 GB')).toBeInTheDocument());
		expect(
			capacity.getByText(/Counts 2 local volumes\. Network shares and disk images/),
		).toBeTruthy();
		expect((await stat('free')).getByText('300 GB')).toBeInTheDocument();
		expect((await stat('volumes')).getByText('3')).toBeInTheDocument();
		expect((await stat('volumes')).getByText('ext4, exfat, cifs')).toBeInTheDocument();
	});

	it('says Home is not measured yet, with the hook the directory-size scan fills', async () => {
		await setup();
		const home = await stat('home');
		expect(home.getByText('Not measured yet')).toBeInTheDocument();
	});
});

describe('a volume card', () => {
	it('has the name, the real file system, a badge, the size and free space', async () => {
		await setup();
		const system = await card('System');
		expect(within(system).getByText('ext4')).toBeInTheDocument();
		expect(within(system).getByText('System', { selector: 'span' })).toBeInTheDocument();
		expect(within(system).getByText('500 GB')).toBeInTheDocument();
		const backup = await card('Backup');
		expect(within(backup).getByText('exfat')).toBeInTheDocument();
		expect(within(backup).getByText('Removable')).toBeInTheDocument();
	});

	it('draws an exact Used / Free bar with a text equivalent and a numeric legend', async () => {
		await setup();
		const backup = await card('Backup');
		const bar = within(backup).getByRole('img');
		expect(bar).toHaveAccessibleName('Backup: 28 GB used and 100 GB free of 128 GB (22% used)');
		const legend = within(backup).getByRole('list', { name: 'Space on Backup' });
		expect(within(legend).getByText('Used').closest('li')).toHaveTextContent('28 GB (22%)');
		expect(within(legend).getByText('Free').closest('li')).toHaveTextContent('100 GB (78%)');
		// The parts differ by pattern, not only colour: each has its own marker.
		expect(bar.querySelector('[data-part="used"]')).not.toBeNull();
	});

	it('leaves a slot for Your files on the volume that holds Home, which says it is not measured yet', async () => {
		await setup();
		const system = await card('System');
		expect(within(system).getByText('Your files: not measured yet')).toBeInTheDocument();
		const backup = await card('Backup');
		expect(within(backup).queryByText(/Your files/)).not.toBeInTheDocument();
	});

	it('paints the cards without waiting for the Trash', async () => {
		const trash = new FakeTrashClient({ count: 1 });
		// A read that never answers.
		trash.getInfo = () => new Promise(() => {});
		await setup({ trash });
		expect(await card('System')).toBeInTheDocument();
		expect(within(await card('Trash')).getByText('Reading the Trash…')).toBeInTheDocument();
	});

	it('shows a nearly full volume as such in words', async () => {
		await setup({ volumes: [fakeVolume('t', { label: 'Tight', total: 1000, free: 20 })] });
		const tight = await card('Tight');
		expect(within(tight).getByRole('img')).toHaveAccessibleName(/Almost full\.$/);
	});

	it('marks the legend of a nearly full volume, so the red hatching of its bar is explained', async () => {
		await setup({
			volumes: [
				fakeVolume('t', { label: 'Tight', total: 1000, free: 20 }),
				fakeVolume('r', { label: 'Roomy', total: 1000, free: 700 }),
			],
		});
		const full = within(await card('Tight')).getByRole('list', { name: /Tight/ });
		const roomy = within(await card('Roomy')).getByRole('list', { name: /Roomy/ });
		expect(full).toHaveAttribute('data-full');
		expect(roomy).not.toHaveAttribute('data-full');
	});

	it('says Size unavailable for a mounted volume whose space cannot be read', async () => {
		await setup({ volumes: [fakeVolume('u', { label: 'Odd', total: null, free: null })] });
		expect(within(await card('Odd')).getByText('Size unavailable')).toBeInTheDocument();
		expect(within(await card('Odd')).queryByRole('img')).not.toBeInTheDocument();
	});
});

describe('network volumes', () => {
	it('are not measured by default and show no bar, only a Measure action', async () => {
		const devices = new FakeDevicesClient([SYSTEM, NAS]);
		await setup({ devices });
		const share = await card('Media share');
		expect(within(share).getByText('Network')).toBeInTheDocument();
		expect(within(share).getByText(/Not measured/)).toBeInTheDocument();
		expect(within(share).queryByRole('img')).not.toBeInTheDocument();
		expect(devices.calls).toEqual([]);
	});

	it('are measured on request, then drawn and announced, and never counted in Capacity', async () => {
		const devices = new FakeDevicesClient([SYSTEM, NAS]);
		devices.measurements.set('nas', { total: 4_000_000_000_000, free: 1_000_000_000_000 });
		await setup({ devices });
		const share = await card('Media share');
		await userEvent.click(within(share).getByRole('button', { name: 'Measure Media share' }));
		await waitFor(() => expect(within(share).getByRole('img')).toBeInTheDocument());
		expect(devices.calls).toEqual([{ action: 'refreshSpace', id: 'nas' }]);
		expect(within(await page()).getByRole('status')).toHaveTextContent('Measured Media share');
		expect((await stat('capacity')).getByText('500 GB')).toBeInTheDocument();
	});

	it('say so when measuring finds nothing', async () => {
		const devices = new FakeDevicesClient([SYSTEM, NAS]);
		await setup({ devices });
		const share = await card('Media share');
		await userEvent.click(within(share).getByRole('button', { name: 'Measure Media share' }));
		const live = within(await page()).getByRole('status');
		await waitFor(() => expect(live).toHaveTextContent('Could not measure Media share.'));
	});
});

describe('a locked volume', () => {
	it('says it is locked and unlocks through the Devices dialog', async () => {
		const devices = new FakeDevicesClient([SYSTEM, VAULT]);
		devices.passphrases.set('vault', 'correct horse');
		await setup({ devices });
		const vault = await card('Vault');
		expect(within(vault).getByText('Locked. Unlock it to see its space.')).toBeInTheDocument();
		await userEvent.click(within(vault).getByRole('button', { name: 'Unlock Vault' }));
		const dialog = await screen.findByRole('dialog', { name: 'Unlock Vault' });
		await userEvent.type(within(dialog).getByLabelText('Passphrase'), 'correct horse{Enter}');
		await waitFor(() =>
			expect(devices.calls.map((call) => call.action)).toEqual(['unlock', 'mount']),
		);
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
		expect(within(await page()).getByRole('status')).toHaveTextContent('Unlocked Vault');
	});

	it('hides Unlock where the plugin cannot unlock', async () => {
		const devices = new FakeDevicesClient([SYSTEM, VAULT], fakeStatus(['unlock']));
		await setup({ devices });
		const vault = await card('Vault');
		await waitFor(() => expect(within(vault).getByText(/^Locked/)).toBeInTheDocument());
		expect(within(vault).queryByRole('button', { name: /Unlock/ })).not.toBeInTheDocument();
	});
});

describe('when volumes cannot be listed', () => {
	it('says why and shows the volume that holds Home from the file system instead', async () => {
		const devices = new FakeDevicesClient([], fakeStatus(['list']));
		await setup({ devices });
		const note = await screen.findByRole('note');
		expect(note).toHaveTextContent('The list of volumes is not available');
		expect(note).toHaveTextContent('list is not supported here');
		const fallback = await card('Home volume');
		expect(within(fallback).getByText('120 GB')).toBeInTheDocument();
		expect(within(fallback).getByText('500 GB')).toBeInTheDocument();
	});

	it('says so without a volumes service at all', async () => {
		await setup({ devices: null });
		const note = await screen.findByRole('note');
		expect(note).toHaveTextContent(t('overview.unavailable.fallback'));
	});
});

describe('opening a volume', () => {
	it('is a button on the name that moves the tab to the mount point, by keyboard too', async () => {
		const h = await setup();
		const backup = await card('Backup');
		const open = within(backup).getByRole('button', { name: 'Open Backup' });
		open.focus();
		await userEvent.keyboard('{Enter}');
		await waitFor(async () => {
			const snapshot = await h.tabs.getSnapshot();
			expect(snapshot.tabs.find((tab) => tab.id === snapshot.active)?.location.uri).toBe(
				'file:///run/media/test/Backup',
			);
		});
	});

	it('has no open button on a volume that is not mounted', async () => {
		await setup({ volumes: [SYSTEM, VAULT] });
		expect(within(await card('Vault')).queryByRole('button', { name: /^Open/ })).toBeNull();
	});
});

describe('the Trash card', () => {
	it('shows the count and, when the plugin could add it up, the size; Open goes to the Trash', async () => {
		const trash = new FakeTrashClient({ count: 3 });
		trash.bytes = 2_500_000;
		const h = await setup({ trash });
		const card_ = await card('Trash');
		await waitFor(() => expect(within(card_).getByText('3 items')).toBeInTheDocument());
		expect(within(card_).getByText('2.5 MB')).toBeInTheDocument();
		// Only Overview asks for the sizes.
		expect(trash.asked).toContain(true);
		await userEvent.click(within(card_).getByRole('button', { name: 'Open Trash' }));
		await waitFor(async () => {
			const snapshot = await h.tabs.getSnapshot();
			expect(snapshot.tabs.find((tab) => tab.id === snapshot.active)?.location.uri).toBe('trash:/');
		});
	});

	it('says the size was not measured rather than showing zero', async () => {
		const trash = new FakeTrashClient({ count: 2 });
		await setup({ trash });
		const card_ = await card('Trash');
		await waitFor(() => expect(within(card_).getByText('2 items')).toBeInTheDocument());
		expect(within(card_).getByText('Not measured')).toBeInTheDocument();
	});

	it('notes that the Trash on other drives has not been verified', async () => {
		await setup({ trash: new FakeTrashClient({ count: 1 }) });
		expect(
			await within(await card('Trash')).findByText(/has not been verified on every kind of drive/),
		).toBeInTheDocument();
	});

	it('empties through the same confirmation as the Trash view, and not for an empty Trash', async () => {
		const trash = new FakeTrashClient({ count: 4 });
		await setup({ trash });
		const card_ = await card('Trash');
		const empty = await within(card_).findByRole('button', { name: 'Empty Trash' });
		await waitFor(() => expect(empty).toBeEnabled());
		await userEvent.click(empty);
		const dialog = await screen.findByRole('dialog', { name: 'Empty the Trash?' });
		expect(dialog).toHaveTextContent('All 4 items in the Trash');
		await userEvent.click(within(dialog).getByRole('button', { name: 'Empty Trash' }));
		await waitFor(() => expect(trash.jobs).toHaveLength(1));
		expect(trash.last.request.kind).toEqual({ kind: 'emptyTrash', olderThanDays: null });
	});

	it('disables Empty while the Trash is empty', async () => {
		await setup({ trash: new FakeTrashClient({ count: 0 }) });
		const card_ = await card('Trash');
		await waitFor(() => expect(within(card_).getByText('Empty', { selector: 'dd' })).toBeTruthy());
		expect(within(card_).getByRole('button', { name: 'Empty Trash' })).toBeDisabled();
	});

	it('says why a Trash that cannot be browsed cannot, and offers neither action', async () => {
		const trash = new FakeTrashClient({
			available: false,
			reason: 'the Trash portal can only move files to the trash',
		});
		await setup({ trash });
		const card_ = await card('Trash');
		await waitFor(() =>
			expect(within(card_).getByText('The Trash cannot be browsed here.')).toBeInTheDocument(),
		);
		expect(
			within(card_).getByText('the Trash portal can only move files to the trash'),
		).toBeTruthy();
		expect(within(card_).queryByRole('button')).not.toBeInTheDocument();
	});

	it('is absent where the window has no Trash service', async () => {
		await setup({ trash: null });
		await card('System');
		expect(within(await page()).queryByRole('group', { name: 'Trash' })).not.toBeInTheDocument();
	});
});

const placesList = () =>
	within(
		within(screen.getByRole('navigation', { name: 'Sidebar' })).getByRole('group', {
			name: 'Places',
		}),
	);

describe('the sidebar and the palette', () => {
	it('has Overview first among the Places, and opening it replaces the folder in the tab', async () => {
		const h = await setup();
		await page();
		// Back on a folder, then through the sidebar.
		const snapshot = await h.tabs.getSnapshot();
		await act(async () => h.tabs.navigate(snapshot.active!, HOME));
		await screen.findByRole('listbox', { name: 'Files' });
		const places = placesList().getAllByRole('button');
		// The first is the section's own heading, the next is Overview and then Home.
		expect(places[1]).toHaveAccessibleName('Overview');
		expect(places[2]).toHaveAccessibleName(/^Home/);
		fireEvent.click(places[1]!);
		expect(await page()).toBeInTheDocument();
		expect(places[1]).toHaveAttribute('aria-current', 'page');
	});

	it('is not a drop target, because it is not a folder', async () => {
		await setup();
		await screen.findByRole('button', { name: /^Home/ });
		const overview = placesList().getByRole('button', { name: 'Overview' });
		expect(Array.from(overview.attributes).some((a) => a.name.startsWith('data-drop'))).toBe(false);
		const home = placesList().getByRole('button', { name: /^Home/ });
		expect(Array.from(home.attributes).some((a) => a.name.startsWith('data-drop'))).toBe(true);
	});

	it('is offered by the palette as Open Overview, wherever the sidebar has the place', () => {
		const command = COMMANDS.find((candidate) => candidate.id === 'openOverview');
		expect(command?.label).toBe('cmd.openOverview');
		expect(t('cmd.openOverview')).toBe('Open Overview');
	});
});
