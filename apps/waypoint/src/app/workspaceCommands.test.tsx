// Verifies the Action bar and the application menu over a whole workspace: the commands act on the active pane and the bar's choices are saved
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { MainOps } from '../ops/MainOps';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakeOsClipboardClient } from '../services/fakeOsClipboardClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { createFakeSettingsClient, type FakeSettings } from '../services/fakeSettingsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { SettingsProvider } from '../settings/SettingsContext';
import { PlacesClientProvider } from '../sidebar/PlacesClientContext';
import { TabsProvider } from '../tabs/TabsContext';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME } from '../test/workspaceHarness';
import { TrashClientProvider } from '../trash/TrashClientContext';
import { dismissNotice } from './notices';
import { AppMenu } from './AppMenu';
import { Workspace } from './Workspace';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
});

async function mount() {
	const vfs = createTree();
	const tabs = new FakeTabsApi();
	await tabs.openTab(HOME);
	const ops = createFakeOpsClient({
		resolveSelection: async (handle, spec) =>
			Promise.all(spec.ids.map((id) => vfs.entryLocation(handle, id))),
	});
	const settings = createFakeSettingsClient();
	const bridge = createCommandBridge();
	render(
		<VfsClientProvider client={vfs}>
			<PlacesClientProvider client={new FakePlacesClient({ places: fakePlaces('/home/test') })}>
				<TrashClientProvider client={undefined}>
					<SettingsProvider client={settings}>
						<TabsProvider api={tabs} home={HOME}>
							<CommandBridgeProvider value={bridge}>
								<AppMenu />
								<MainOps client={ops} osClipboard={new FakeOsClipboardClient()}>
									<Workspace />
								</MainOps>
							</CommandBridgeProvider>
						</TabsProvider>
					</SettingsProvider>
				</TrashClientProvider>
			</PlacesClientProvider>
		</VfsClientProvider>,
	);
	fireEvent.keyDown(window, { key: 'F9' });
	return { vfs, ops, settings, bridge };
}

const row = (name: string) =>
	within(screen.getByRole('listbox', { name: 'Files' }))
		.getAllByRole('option')
		.find((option) => option.textContent?.includes(name))!;
const submits = (ops: FakeOpsClient) => ops.calls.filter((call) => call[0] === 'submit');
const bar = () => screen.queryByRole('toolbar', { name: 'Actions' });
const barButton = (name: string) => within(bar()!).getByRole('button', { name });
/** What the bar last asked Rust to change: only its own field, never the whole document. */
const lastUiChange = (settings: FakeSettings) => settings.uiCalls.at(-1)!;

async function choose(name: string) {
	await waitFor(() => expect(row(name)).toBeDefined());
	fireEvent.click(row(name));
	await waitFor(() => expect(row(name)).toHaveAttribute('aria-selected', 'true'));
}

function openMenu(menu: string) {
	fireEvent.keyDown(window, { key: menu, altKey: true });
}

describe('the Action bar over a workspace', () => {
	it('is on under the toolbar by default', async () => {
		await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		expect(barButton('New')).toBeEnabled();
	});

	it('enables Copy with a selection, and Copy then Paste make a copy job', async () => {
		const { ops } = await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		expect(barButton('Copy')).toHaveAttribute('aria-disabled', 'true');
		expect(barButton('Paste')).toHaveAttribute('aria-disabled', 'true');
		await choose('notes.txt');
		await waitFor(() => expect(barButton('Copy')).not.toHaveAttribute('aria-disabled'));
		await userEvent.click(barButton('Copy'));
		await waitFor(() => expect(barButton('Paste')).not.toHaveAttribute('aria-disabled'));
		fireEvent.doubleClick(row('music'));
		await waitFor(() => expect(row('song.mp3')).toBeDefined());
		await userEvent.click(barButton('Paste'));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(submits(ops)[0]![1]).toMatchObject({
			kind: { kind: 'copy' },
			destination: { uri: 'file:///home/test/music' },
		});
	});

	it('makes a folder from New and moves the selection to the Trash from Delete', async () => {
		const { ops } = await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		await userEvent.click(barButton('New'));
		await userEvent.click(await screen.findByRole('menuitem', { name: /New Folder/ }));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(submits(ops)[0]![1]).toMatchObject({ kind: { kind: 'createFolder' } });
		await choose('notes.txt');
		await waitFor(() => expect(barButton('Delete')).not.toHaveAttribute('aria-disabled'));
		await userEvent.click(barButton('Delete'));
		// The confirmation is a setting that is on by default.
		const confirm = await screen.findByRole('button', { name: 'Move to Trash' }).catch(() => null);
		if (confirm) await userEvent.click(confirm);
		await waitFor(() =>
			expect(
				submits(ops).some((call) => (call[1] as { kind: { kind: string } }).kind.kind === 'trash'),
			).toBe(true),
		);
	});

	it('undoes from the bar after a change', async () => {
		const { ops } = await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		await userEvent.click(barButton('New'));
		await userEvent.click(await screen.findByRole('menuitem', { name: /New Folder/ }));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		await act(async () => {
			ops.start(ops.jobs()[0]!.id);
			ops.done(ops.jobs()[0]!.id, 'New folder');
		});
		await waitFor(() => expect(barButton('Undo')).not.toHaveAttribute('aria-disabled'));
		await userEvent.click(barButton('Undo'));
		await waitFor(() => expect(ops.calls.some((call) => call[0] === 'undo')).toBe(true));
	});
});

describe('hiding the bar and its labels', () => {
	it('saves Hide Labels to the settings and shows icons only once Rust confirms', async () => {
		const { settings } = await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		expect(barButton('Cut')).toHaveTextContent('Cut');
		fireEvent.contextMenu(bar()!);
		await userEvent.click(screen.getByRole('menuitem', { name: 'Hide Labels' }));
		await waitFor(() => expect(lastUiChange(settings)).toEqual({ actionBarLabels: false }));
		expect(settings.calls).toEqual([]);
		expect(settings.current().settings.ui).toEqual({
			actionBar: true,
			actionBarLabels: false,
			appNameInTitle: false,
		});
		await waitFor(() => expect(barButton('Cut')).toHaveTextContent(''));
	});

	it('saves Hide Action Bar, and View → Action Bar brings it back', async () => {
		const { settings } = await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		fireEvent.contextMenu(bar()!);
		await userEvent.click(screen.getByRole('menuitem', { name: 'Hide Action Bar' }));
		await waitFor(() => expect(bar()).not.toBeInTheDocument());
		expect(lastUiChange(settings)).toEqual({ actionBar: false });
		expect(settings.calls).toEqual([]);

		openMenu('v');
		const item = await screen.findByRole('menuitemcheckbox', { name: /Action Bar/ });
		expect(item).toHaveAttribute('aria-checked', 'false');
		await userEvent.click(item);
		await waitFor(() => expect(bar()).toBeInTheDocument());
		expect(lastUiChange(settings)).toEqual({ actionBar: true });
	});

	it('follows a change another window makes', async () => {
		const { settings } = await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		const current = settings.current().settings;
		act(() => {
			settings.change({
				...current,
				ui: { actionBar: false, actionBarLabels: true, appNameInTitle: false },
			});
		});
		await waitFor(() => expect(bar()).not.toBeInTheDocument());
	});

	it('keeps what the settings document says after a restart: a new window starts from it', async () => {
		cleanup();
		const vfs = createTree();
		const tabs = new FakeTabsApi();
		await tabs.openTab(HOME);
		const settings = createFakeSettingsClient({
			...createFakeSettingsClient().current().settings,
			ui: { actionBar: true, actionBarLabels: false, appNameInTitle: false },
		});
		render(
			<VfsClientProvider client={vfs}>
				<PlacesClientProvider client={new FakePlacesClient({ places: fakePlaces('/home/test') })}>
					<TrashClientProvider client={undefined}>
						<SettingsProvider client={settings}>
							<TabsProvider api={tabs} home={HOME}>
								<Workspace />
							</TabsProvider>
						</SettingsProvider>
					</TrashClientProvider>
				</PlacesClientProvider>
			</VfsClientProvider>,
		);
		await waitFor(() => expect(bar()).toBeInTheDocument());
		await waitFor(() => expect(bar()).toHaveAttribute('data-labels', 'false'));
	});
});

describe('the application menu over a workspace', () => {
	it('lists the undo history after a change and undoes it from Edit', async () => {
		const { ops } = await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		await userEvent.click(barButton('New'));
		await userEvent.click(await screen.findByRole('menuitem', { name: /New Folder/ }));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		await act(async () => {
			ops.start(ops.jobs()[0]!.id);
			ops.done(ops.jobs()[0]!.id, 'New folder');
		});
		openMenu('e');
		const undo = await screen.findByRole('menuitem', { name: /Undo New folder/ });
		await userEvent.click(
			within(await screen.findByRole('menu', { name: 'Edit' })).getByRole('menuitem', {
				name: /Undo History/,
			}),
		);
		const history = within(await screen.findByRole('menu', { name: 'Undo History' }));
		expect(history.getAllByRole('menuitem')[0]).toHaveTextContent('New folder');
		expect(undo).not.toHaveAttribute('aria-disabled');
		await userEvent.click(history.getAllByRole('menuitem')[0]!);
		await waitFor(() => expect(ops.calls.some((call) => call[0] === 'undo')).toBe(true));
	});

	it('is disabled for what has no selection and enabled once there is one', async () => {
		await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		openMenu('f');
		const trash = await screen.findByRole('menuitem', { name: /Move to Trash/ });
		expect(trash).toHaveAttribute('aria-disabled', 'true');
		// Escape closes the submenu, then the menu.
		fireEvent.keyDown(trash, { key: 'Escape' });
		fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		await choose('notes.txt');
		await waitFor(() => expect(barButton('Delete')).not.toHaveAttribute('aria-disabled'));
		openMenu('f');
		expect(await screen.findByRole('menuitem', { name: /Move to Trash/ })).not.toHaveAttribute(
			'aria-disabled',
		);
	});

	it('switches the view from View → Grid and the bar follows', async () => {
		await mount();
		await waitFor(() => expect(bar()).toBeInTheDocument());
		expect(barButton('View')).toHaveAttribute('title', expect.stringContaining('Grid'));
		openMenu('v');
		await userEvent.click(await screen.findByRole('menuitemcheckbox', { name: /Grid/ }));
		await waitFor(() =>
			expect(barButton('View')).toHaveAttribute('title', expect.stringContaining('List')),
		);
	});
});
