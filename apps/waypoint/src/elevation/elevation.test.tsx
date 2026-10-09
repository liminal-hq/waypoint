// Verifies Open as Administrator over a whole workspace: the entry points, the waiting state and its Cancel, the new tab, the marker, and the states of a folder that needs approval
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PluginStatus } from '@liminal-hq/plugin-elevate';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dismissNotice } from '../app/notices';
import { AppMenu } from '../app/AppMenu';
import { Workspace } from '../app/Workspace';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { ConnectionsProvider } from '../connections/ConnectionsContext';
import { FakeConnectionsClient } from '../connections/fakeConnectionsClient';
import { MainOps } from '../ops/MainOps';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { FakeOsClipboardClient } from '../services/fakeOsClipboardClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { fileLocation, makeEntry } from '../services/fakeVfsClient';
import { DEFAULT_SETTINGS, type Settings } from '../services/settingsClient';
import { SettingsProvider } from '../settings/SettingsContext';
import { PlacesClientProvider } from '../sidebar/PlacesClientContext';
import { TabsProvider } from '../tabs/TabsContext';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME } from '../test/workspaceHarness';
import { TrashClientProvider } from '../trash/TrashClientContext';
import { ElevatedBadge } from './ElevatedBadge';
import { ElevationProvider } from './ElevationContext';
import { elevatedLocation } from './elevatedLocation';
import { elevationStore } from './elevationStore';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
});

const WORKS: PluginStatus = {
	available: true,
	reason: null,
	flavour: 'polkit',
	features: [{ name: 'elevate', available: true, reason: null }],
};

const withSetting = (administratorAccess: boolean): Settings => ({
	...DEFAULT_SETTINGS,
	experimental: { ...DEFAULT_SETTINGS.experimental, administratorAccess },
});

const ADMIN_HOME = elevatedLocation(HOME);

interface MountOptions {
	setting?: boolean;
	status?: () => Promise<PluginStatus>;
	connections?: FakeConnectionsClient;
	/** The location the one tab opens at. */
	at?: typeof HOME;
	prepare?: (vfs: ReturnType<typeof createTree>) => void;
}

async function mount(options: MountOptions = {}) {
	const vfs = createTree();
	vfs.setFolder(ADMIN_HOME, [
		makeEntry(1, 'secrets', { kind: 'directory' }),
		makeEntry(2, 'shadow'),
		makeEntry(3, 'passwd'),
	]);
	options.prepare?.(vfs);
	const tabs = new FakeTabsApi();
	await tabs.openTab(options.at ?? HOME);
	const ops = createFakeOpsClient({
		resolveSelection: async (handle, spec) =>
			Promise.all(spec.ids.map((id) => vfs.entryLocation(handle, id))),
	});
	const settings = createFakeSettingsClient(withSetting(options.setting ?? true));
	const connections = options.connections ?? new FakeConnectionsClient({ schemes: [] });
	const bridge = createCommandBridge();
	render(
		<VfsClientProvider client={vfs}>
			<PlacesClientProvider client={new FakePlacesClient({ places: fakePlaces('/home/test') })}>
				<TrashClientProvider client={undefined}>
					<SettingsProvider client={settings}>
						<ConnectionsProvider client={connections}>
							<ElevationProvider status={options.status ?? (() => Promise.resolve(WORKS))}>
								<TabsProvider api={tabs} home={HOME}>
									<CommandBridgeProvider value={bridge}>
										<AppMenu />
										<ElevatedBadge />
										<MainOps client={ops} osClipboard={new FakeOsClipboardClient()}>
											<Workspace />
										</MainOps>
									</CommandBridgeProvider>
								</TabsProvider>
							</ElevationProvider>
						</ConnectionsProvider>
					</SettingsProvider>
				</TrashClientProvider>
			</PlacesClientProvider>
		</VfsClientProvider>,
	);
	return { vfs, tabs, ops, settings, connections, bridge };
}

const row = (name: string) =>
	within(screen.getByRole('listbox', { name: 'Files' }))
		.getAllByRole('option')
		.find((option) => option.textContent?.includes(name))!;
const openFileMenu = () => fireEvent.keyDown(window, { key: 'f', altKey: true });
const connectCalls = (connections: FakeConnectionsClient) =>
	connections.calls.filter((call) => call.method === 'connect');
const activeTab = async (tabs: FakeTabsApi) => {
	const snapshot = await tabs.getSnapshot();
	return snapshot.tabs.find((tab) => tab.id === snapshot.active)!;
};

async function chooseFromFileMenu(name: string) {
	fireEvent.keyDown(window, { key: 'F9' });
	openFileMenu();
	await userEvent.click(await screen.findByRole('menuitem', { name }));
}

describe('Open as Administrator in the File menu', () => {
	it('connects first, shows the waiting state with Cancel, and opens a new tab only once the prompt is answered', async () => {
		const connections = new FakeConnectionsClient({ schemes: [] });
		connections.holdConnects = true;
		const { tabs } = await mount({ connections });
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		await chooseFromFileMenu('Open as Administrator');
		await waitFor(() => expect(connectCalls(connections)).toHaveLength(1));
		expect(connectCalls(connections)[0]!.args[0]).toEqual(ADMIN_HOME);
		// Waiting, not blocking: a status with a Cancel button, and still one tab.
		expect(await screen.findByText('Waiting for the system’s prompt…')).toBeVisible();
		expect(screen.getByRole('button', { name: 'Cancel' })).toBeEnabled();
		expect((await tabs.getSnapshot()).tabs).toHaveLength(1);
		await act(async () => connections.releaseConnects(null));
		await waitFor(async () => expect((await tabs.getSnapshot()).tabs).toHaveLength(2));
		expect((await activeTab(tabs)).location).toEqual(ADMIN_HOME);
		await waitFor(() => expect(screen.queryByText('Waiting for the system’s prompt…')).toBeNull());
	});

	it('stops the wait from Cancel: the connect is cancelled, no tab opens and nothing is said', async () => {
		const connections = new FakeConnectionsClient({ schemes: [] });
		connections.holdConnects = true;
		const { tabs } = await mount({ connections });
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		await chooseFromFileMenu('Open as Administrator');
		await userEvent.click(await screen.findByRole('button', { name: 'Cancel' }));
		await waitFor(() =>
			expect(connections.calls.some((call) => call.method === 'cancelConnect')).toBe(true),
		);
		await waitFor(() => expect(elevationStore.getState().pending).toBeNull());
		expect(screen.queryByText('Waiting for the system’s prompt…')).toBeNull();
		expect((await tabs.getSnapshot()).tabs).toHaveLength(1);
		expect(screen.queryByText('Authentication was cancelled or refused')).toBeNull();
	});

	it('says the prompt was cancelled or refused, and opens no tab', async () => {
		const refused = { kind: 'authFailed', location: ADMIN_HOME } as unknown as VfsError;
		const connections = new FakeConnectionsClient({ schemes: [], connect: () => refused });
		const { tabs } = await mount({ connections });
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		await chooseFromFileMenu('Open as Administrator');
		expect(await screen.findByText('Authentication was cancelled or refused')).toBeVisible();
		expect((await tabs.getSnapshot()).tabs).toHaveLength(1);
	});

	it('gives the reason when the helper cannot start', async () => {
		const unsupported: VfsError = { kind: 'unsupported', what: 'the helper is missing' };
		const connections = new FakeConnectionsClient({ schemes: [], connect: () => unsupported });
		await mount({ connections });
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		await chooseFromFileMenu('Open as Administrator');
		expect(
			await screen.findByText('Administrator access is not available: the helper is missing'),
		).toBeVisible();
	});

	it('opens the one selected folder, not the current one', async () => {
		const connections = new FakeConnectionsClient({ schemes: [] });
		const { tabs } = await mount({ connections });
		await waitFor(() => expect(row('docs')).toBeDefined());
		fireEvent.click(row('docs'));
		await waitFor(() => expect(row('docs')).toHaveAttribute('aria-selected', 'true'));
		await chooseFromFileMenu('Open as Administrator');
		await waitFor(async () => expect((await tabs.getSnapshot()).tabs).toHaveLength(2));
		expect((await activeTab(tabs)).location.uri).toBe('admin:///home/test/docs');
	});

	it('is disabled with a reason when a file is selected', async () => {
		await mount();
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		fireEvent.click(row('notes.txt'));
		await waitFor(() => expect(row('notes.txt')).toHaveAttribute('aria-selected', 'true'));
		fireEvent.keyDown(window, { key: 'F9' });
		openFileMenu();
		const item = await screen.findByRole('menuitem', { name: 'Open as Administrator' });
		expect(item).toHaveAttribute('aria-disabled', 'true');
	});

	it.each([
		['the setting is off', { setting: false }],
		[
			'the plugin cannot start a helper',
			{ status: () => Promise.resolve({ ...WORKS, available: false }) },
		],
		['the status cannot be read', { status: () => Promise.reject(new Error('no plugin')) }],
	] as const)('is not offered when %s', async (_why, options) => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		await mount(options);
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		fireEvent.keyDown(window, { key: 'F9' });
		openFileMenu();
		await screen.findByRole('menuitem', { name: /New Folder/ });
		expect(screen.queryByRole('menuitem', { name: 'Open as Administrator' })).toBeNull();
		warn.mockRestore();
	});
});

describe('Administrator Mode in an elevated tab', () => {
	async function elevatedTab() {
		const mounted = await mount({ at: ADMIN_HOME });
		await waitFor(() => expect(row('shadow')).toBeDefined());
		return mounted;
	}

	it('shows the title-bar badge as a button named Administrator that leaves the mode', async () => {
		const { tabs } = await elevatedTab();
		const badge = await screen.findByRole('button', { name: 'Administrator' });
		expect(badge).toHaveAccessibleDescription('Leave Administrator Mode');
		await userEvent.click(badge);
		await waitFor(async () =>
			expect((await activeTab(tabs)).location.uri).toBe('file:///home/test'),
		);
		// The same tab, so its history is kept; the badge goes with the mode.
		expect((await tabs.getSnapshot()).tabs).toHaveLength(1);
		await waitFor(() => expect(screen.queryByRole('button', { name: 'Administrator' })).toBeNull());
	});

	it('shows no badge in an ordinary tab', async () => {
		await mount();
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
		expect(screen.queryByRole('button', { name: 'Administrator' })).toBeNull();
	});

	it('swaps Open as Administrator for Leave Administrator Mode in the File menu, and hides Move to Trash', async () => {
		await elevatedTab();
		fireEvent.keyDown(window, { key: 'F9' });
		fireEvent.click(row('shadow'));
		await waitFor(() => expect(row('shadow')).toHaveAttribute('aria-selected', 'true'));
		openFileMenu();
		expect(await screen.findByRole('menuitem', { name: 'Leave Administrator Mode' })).toBeVisible();
		expect(screen.queryByRole('menuitem', { name: 'Open as Administrator' })).toBeNull();
		expect(screen.queryByRole('menuitem', { name: 'Move to Trash' })).toBeNull();
		expect(screen.getByRole('menuitem', { name: /Delete Permanently/ })).toBeVisible();
	});

	it('does nothing, quietly, when Enter or a double-click opens a file, and still opens a folder', async () => {
		const { vfs } = await elevatedTab();
		fireEvent.doubleClick(row('shadow'));
		fireEvent.click(row('passwd'));
		fireEvent.keyDown(row('passwd'), { key: 'Enter' });
		await act(async () => {});
		expect(vfs.opened).toEqual([]);
		expect(document.querySelector('[data-notice]')).toBeNull();
		vfs.setFolder({ display: '/home/test/secrets', uri: 'admin:///home/test/secrets' }, [
			makeEntry(1, 'inner.txt'),
		]);
		fireEvent.doubleClick(row('secrets'));
		await waitFor(() => expect(row('inner.txt')).toBeDefined());
	});

	it('shows the Administrator word and a shield on the tab, and says it in the tab’s details', async () => {
		await elevatedTab();
		const tab = (await screen.findAllByRole('tab')).find((candidate) =>
			candidate.closest('[data-elevated]'),
		)!;
		expect(tab).toBeDefined();
		expect(tab).toHaveAccessibleDescription(/Administrator/);
		expect(tab.getAttribute('title')).toContain('Administrator');
	});
});

describe('a folder that needs approval', () => {
	const disconnected = (location: typeof ADMIN_HOME): VfsError => ({
		kind: 'disconnected',
		location,
	});

	it('asks for approval with Authenticate and Leave Administrator Mode, and prompts only when chosen', async () => {
		const connections = new FakeConnectionsClient({ schemes: [] });
		const { vfs } = await mount({
			at: ADMIN_HOME,
			connections,
			prepare: (fake) => fake.failOpening(ADMIN_HOME, disconnected(ADMIN_HOME)),
		});
		expect(
			await screen.findByText(
				'Administrator access needs your approval before it shows this folder.',
			),
		).toBeVisible();
		expect(screen.getByRole('button', { name: 'Leave Administrator Mode' })).toBeVisible();
		// Restoring and opening never prompt.
		expect(connectCalls(connections)).toHaveLength(0);
		vfs.succeedOpening(ADMIN_HOME);
		await userEvent.click(screen.getByRole('button', { name: 'Authenticate' }));
		await waitFor(() => expect(connectCalls(connections)).toHaveLength(1));
		// After a successful connect the listing reloads.
		await waitFor(() => expect(row('shadow')).toBeDefined());
	});

	it('shows the waiting state with Cancel while the prompt is up, and says a refusal', async () => {
		const connections = new FakeConnectionsClient({ schemes: [] });
		connections.holdConnects = true;
		await mount({
			at: ADMIN_HOME,
			connections,
			prepare: (fake) => fake.failOpening(ADMIN_HOME, disconnected(ADMIN_HOME)),
		});
		await userEvent.click(await screen.findByRole('button', { name: 'Authenticate' }));
		expect(await screen.findByText('Waiting for the system’s prompt…')).toBeVisible();
		await act(async () =>
			connections.releaseConnects({
				kind: 'authFailed',
				location: ADMIN_HOME,
			} as unknown as VfsError),
		);
		expect(await screen.findByText('Authentication was cancelled or refused')).toBeVisible();
		expect(screen.getByRole('button', { name: 'Authenticate' })).toBeEnabled();
	});

	it('leaves Administrator Mode from the state, to the same folder in the ordinary way', async () => {
		const { tabs } = await mount({
			at: ADMIN_HOME,
			prepare: (fake) => fake.failOpening(ADMIN_HOME, disconnected(ADMIN_HOME)),
		});
		await userEvent.click(await screen.findByRole('button', { name: 'Leave Administrator Mode' }));
		await waitFor(async () =>
			expect((await activeTab(tabs)).location.uri).toBe('file:///home/test'),
		);
		await waitFor(() => expect(row('notes.txt')).toBeDefined());
	});
});

describe('a folder that is denied', () => {
	const ROOT = fileLocation('/root');
	const denied = (location: typeof ROOT): VfsError => ({ kind: 'permissionDenied', location });

	it('offers Open as Administrator, which asks for the prompt for that folder', async () => {
		const connections = new FakeConnectionsClient({ schemes: [] });
		const { tabs } = await mount({
			at: ROOT,
			connections,
			prepare: (fake) => fake.failOpening(ROOT, denied(ROOT)),
		});
		const button = await screen.findByRole('button', { name: 'Open as Administrator' });
		await userEvent.click(button);
		await waitFor(() => expect(connectCalls(connections)).toHaveLength(1));
		expect(connectCalls(connections)[0]!.args[0]).toEqual(elevatedLocation(ROOT));
		await waitFor(async () => expect((await tabs.getSnapshot()).tabs).toHaveLength(2));
	});

	it('offers nothing when administrator access is off', async () => {
		await mount({
			at: ROOT,
			setting: false,
			prepare: (fake) => fake.failOpening(ROOT, denied(ROOT)),
		});
		expect(await screen.findByText('Permission denied')).toBeVisible();
		expect(screen.queryByRole('button', { name: 'Open as Administrator' })).toBeNull();
	});

	it('offers nothing for a folder that is not on this computer', async () => {
		const server = { display: 'sftp://me@nas.lan/root', uri: 'sftp://me@nas.lan/root' };
		await mount({
			at: server,
			prepare: (fake) => fake.failOpening(server, denied(server)),
		});
		expect(await screen.findByText('Permission denied')).toBeVisible();
		expect(screen.queryByRole('button', { name: 'Open as Administrator' })).toBeNull();
	});
});
