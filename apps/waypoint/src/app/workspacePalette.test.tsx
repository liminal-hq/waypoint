// Verifies the command palette over a whole workspace: the key and the menu open it, and what it runs is what the window does
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { CommandPaletteHost } from '../commands/CommandPaletteHost';
import { keyEventInit } from '../commands/shortcuts';
import { t } from '../i18n/messages';
import { createRequest } from '../ops/fileCommands';
import { MainOps } from '../ops/MainOps';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakeOsClipboardClient } from '../services/fakeOsClipboardClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
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
	const bridge = createCommandBridge();
	render(
		<VfsClientProvider client={vfs}>
			<PlacesClientProvider client={new FakePlacesClient({ places: fakePlaces('/home/test') })}>
				<TrashClientProvider client={undefined}>
					<SettingsProvider client={createFakeSettingsClient()}>
						<TabsProvider api={tabs} home={HOME}>
							<CommandBridgeProvider value={bridge}>
								<AppMenu />
								<MainOps client={ops} osClipboard={new FakeOsClipboardClient()}>
									<Workspace />
								</MainOps>
								<CommandPaletteHost />
							</CommandBridgeProvider>
						</TabsProvider>
					</SettingsProvider>
				</TrashClientProvider>
			</PlacesClientProvider>
		</VfsClientProvider>,
	);
	return { tabs, ops, bridge };
}

const submits = (ops: FakeOpsClient) => ops.calls.filter((call) => call[0] === 'submit');
const openPalette = () => fireEvent.keyDown(window, keyEventInit('Ctrl+Shift+P'));
const combobox = () => screen.findByRole('combobox');

/** A change the journal now holds: the queue runs `label`'s job to its end. */
async function change(ops: FakeOpsClient, label: string) {
	await act(async () => {
		const id = await ops.submit(createRequest('createFolder', HOME, label, 'main-1'));
		ops.start(id);
		ops.done(id, label);
	});
}

describe('opening the palette in a window', () => {
	it('opens from Ctrl+Shift+P and from View → Command Palette…', async () => {
		await mount();
		await waitFor(() => expect(screen.getByRole('listbox', { name: 'Files' })).toBeInTheDocument());
		openPalette();
		expect(await combobox()).toBeInTheDocument();
		await userEvent.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

		fireEvent.keyDown(window, { key: 'v', altKey: true });
		await userEvent.click(await screen.findByRole('menuitem', { name: /Command Palette…/ }));
		expect(await combobox()).toBeInTheDocument();
	});

	it('runs New Folder as the menu would: a create job in the queue', async () => {
		const { ops } = await mount();
		await waitFor(() => expect(screen.getByRole('listbox', { name: 'Files' })).toBeInTheDocument());
		openPalette();
		await userEvent.type(await combobox(), 'new folder{Enter}');
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(submits(ops)[0]![1]).toMatchObject({ kind: { kind: 'createFolder' } });
	});

	it('offers Go to Downloads and opens that folder in the active tab', async () => {
		const { tabs } = await mount();
		await waitFor(() => expect(screen.getByRole('listbox', { name: 'Files' })).toBeInTheDocument());
		openPalette();
		await userEvent.type(await combobox(), 'go to downloads{Enter}');
		await waitFor(async () => {
			const snapshot = await tabs.getSnapshot();
			const active = snapshot.tabs.find((tab) => tab.id === snapshot.active);
			expect(active?.location.uri).toBe('file:///home/test/Downloads');
		});
	});

	it('says why a command that needs a selection cannot run', async () => {
		await mount();
		await waitFor(() => expect(screen.getByRole('listbox', { name: 'Files' })).toBeInTheDocument());
		openPalette();
		await userEvent.type(await combobox(), 'move to trash{Enter}');
		const dialog = screen.getByRole('dialog');
		expect(within(dialog).getByRole('status')).toHaveTextContent(t('cmd.reason.nothingSelected'));
	});
});

describe('the undo history in the palette', () => {
	it('undoes back to an older entry after a confirmation, newest first, one job per entry', async () => {
		const { ops } = await mount();
		await waitFor(() => expect(screen.getByRole('listbox', { name: 'Files' })).toBeInTheDocument());
		await change(ops, 'First');
		await change(ops, 'Second');
		await change(ops, 'Third');
		await waitFor(() => expect(ops.jobs()).toHaveLength(3));
		openPalette();
		await userEvent.type(await combobox(), 'undo');
		const older = await screen.findByRole('option', { name: /^Undo 3 changes back to: First/ });
		await userEvent.click(older);
		const dialog = await screen.findByRole('dialog', { name: 'Undo 3 changes?' });
		expect(within(dialog).getByRole('button', { name: t('files.cancel') })).toHaveFocus();
		await userEvent.click(
			within(dialog).getByRole('button', { name: t('history.confirm.undo.confirm') }),
		);

		// Each undo is its own job; the queue runs them one after the other.
		for (let step = 0; step < 3; step++) {
			await waitFor(() => expect(ops.jobs()).toHaveLength(4 + step));
			await act(async () => {
				const job = ops.jobs()[3 + step]!;
				ops.start(job.id);
				ops.done(job.id);
			});
		}
		const undone = ops.calls.filter((call) => call[0] === 'undo').map((call) => call[1]);
		expect(undone).toHaveLength(3);
		// The newest entry has the highest id in the journal, and goes first.
		expect([...undone].sort((a, b) => (b as number) - (a as number))).toEqual(undone);
		await waitFor(() =>
			expect(screen.getByText('Undid 3 changes, back to: First')).toBeInTheDocument(),
		);
	});
});
