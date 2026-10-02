// Verifies Properties in a window from the browsing window: Alt+Enter, the item menu, the palette and the Inspector's button; one window per subject, four at most
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { Workspace } from '../app/Workspace';
import { dismissNotice } from '../app/notices';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { CommandPaletteHost } from '../commands/CommandPaletteHost';
import { keyEventInit } from '../commands/shortcuts';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { SettingsProvider } from '../settings/SettingsContext';
import { PlacesClientProvider } from '../sidebar/PlacesClientContext';
import { TabsProvider } from '../tabs/TabsContext';
import { TrashClientProvider } from '../trash/TrashClientContext';
import { fakeDetails } from '../services/fakeDetailsClient';
import { FakePropertiesWindowClient } from '../services/fakePropertiesWindowClient';
import { fileLocation } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME, renderWorkspace } from '../test/workspaceHarness';
import { LazyDetails } from './inspectorHarness';
import { PropertiesWindowProvider } from './PropertiesWindowContext';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
});

const option = (name: string) => screen.findByRole('option', { name: new RegExp(`^${name}`) });
const altEnter = (target: Element | Window) =>
	fireEvent.keyDown(target, { key: 'Enter', altKey: true });
const uris = (client: FakePropertiesWindowClient) =>
	[...client.windows.values()].map((location) => location.uri);

function details() {
	return new LazyDetails({
		3: { details: fakeDetails({ name: 'notes.txt', unavailable: [] }) },
	});
}

async function mount(windows: FakePropertiesWindowClient | undefined) {
	await renderWorkspace(createTree(), undefined, undefined, {
		details: details(),
		...(windows ? { propertiesWindow: windows } : {}),
	});
	await option('notes.txt');
}

describe('Properties in a window', () => {
	it('opens a window for the selected entry on Alt+Enter, and does not open the entry', async () => {
		const windows = new FakePropertiesWindowClient();
		await mount(windows);
		const row = await option('docs');
		fireEvent.click(row);
		altEnter(screen.getByRole('listbox'));
		await waitFor(() => expect(uris(windows)).toEqual([fileLocation('/home/test/docs').uri]));
		// Enter alone would have opened the folder; the list is still the home folder's.
		expect(await option('notes.txt')).toBeInTheDocument();
	});

	it('opens a window for the folder when nothing is selected', async () => {
		const windows = new FakePropertiesWindowClient();
		await mount(windows);
		altEnter(screen.getByRole('listbox'));
		await waitFor(() => expect(uris(windows)).toEqual([HOME.uri]));
	});

	it('brings the window that has the subject to the front instead of making a second', async () => {
		const windows = new FakePropertiesWindowClient();
		await mount(windows);
		fireEvent.click(await option('notes.txt'));
		altEnter(screen.getByRole('listbox'));
		await waitFor(() => expect(windows.windows.size).toBe(1));
		altEnter(screen.getByRole('listbox'));
		await waitFor(() => expect(windows.focused).toEqual(['properties-1']));
		expect(windows.windows.size).toBe(1);
	});

	it('refuses a fifth window with a notice', async () => {
		const windows = new FakePropertiesWindowClient();
		await mount(windows);
		const list = screen.getByRole('listbox');
		for (const name of ['docs', 'music', 'notes.txt']) {
			fireEvent.click(await option(name));
			altEnter(list);
			await waitFor(() => expect(windows.windows.size).toBeGreaterThan(0));
		}
		// Nothing selected: the folder itself is the fourth subject.
		fireEvent.keyDown(list, { key: 'Escape' });
		altEnter(list);
		await waitFor(() => expect(windows.windows.size).toBe(4));
		fireEvent.click(await option('photo.jpg'));
		altEnter(list);
		expect(
			await screen.findByText(
				'Four Properties windows are open already. Close one to open another.',
			),
		).toBeInTheDocument();
		expect(windows.windows.size).toBe(4);
	});

	it('does nothing with several items selected, where a window would be about no one thing', async () => {
		const windows = new FakePropertiesWindowClient();
		await mount(windows);
		const list = screen.getByRole('listbox');
		fireEvent.click(await option('notes.txt'));
		fireEvent.keyDown(list, { key: 'a', ctrlKey: true });
		altEnter(list);
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(windows.windows.size).toBe(0);
	});

	it('is not offered without the service: Alt+Enter does nothing and the menu has no item', async () => {
		await mount(undefined);
		const list = screen.getByRole('listbox');
		fireEvent.click(await option('notes.txt'));
		altEnter(list);
		fireEvent.contextMenu(await option('notes.txt'));
		const menu = await screen.findByRole('menu', { name: 'Item actions' });
		expect(within(menu).queryByRole('menuitem', { name: 'Properties in a Window' })).toBeNull();
		expect(within(menu).getByRole('menuitem', { name: 'Properties' })).toBeInTheDocument();
	});

	it('opens one from the item menu for the entry that was right-clicked', async () => {
		const windows = new FakePropertiesWindowClient();
		await mount(windows);
		fireEvent.contextMenu(await option('photo.jpg'));
		const menu = await screen.findByRole('menu', { name: 'Item actions' });
		const item = within(menu).getByRole('menuitem', { name: /Properties in a Window/ });
		expect(item).toHaveTextContent('Alt+Enter');
		fireEvent.click(item);
		await waitFor(() => expect(uris(windows)).toEqual([fileLocation('/home/test/photo.jpg').uri]));
	});

	it('opens one from the command palette', async () => {
		const windows = new FakePropertiesWindowClient();
		const vfs = createTree();
		const tabs = new FakeTabsApi();
		await tabs.openTab(HOME);
		render(
			<VfsClientProvider client={vfs}>
				<PlacesClientProvider client={new FakePlacesClient({ places: fakePlaces('/home/test') })}>
					<TrashClientProvider client={undefined}>
						<SettingsProvider client={createFakeSettingsClient()}>
							<PropertiesWindowProvider client={windows}>
								<TabsProvider api={tabs} home={HOME}>
									<CommandBridgeProvider value={createCommandBridge()}>
										<Workspace />
										<CommandPaletteHost />
									</CommandBridgeProvider>
								</TabsProvider>
							</PropertiesWindowProvider>
						</SettingsProvider>
					</TrashClientProvider>
				</PlacesClientProvider>
			</VfsClientProvider>,
		);
		fireEvent.click(await option('music'));
		fireEvent.keyDown(window, keyEventInit('Ctrl+Shift+P'));
		const input = await screen.findByRole('combobox');
		await userEvent.type(input, 'properties in');
		await userEvent.keyboard('{Enter}');
		await waitFor(() => expect(uris(windows)).toEqual([fileLocation('/home/test/music').uri]));
	});

	it('opens one from the Inspector’s Properties tab, for what it shows', async () => {
		const windows = new FakePropertiesWindowClient();
		await mount(windows);
		fireEvent.keyDown(window, { key: 'F11' });
		fireEvent.click(await screen.findByRole('tab', { name: 'Properties' }));
		fireEvent.click(await option('notes.txt'));
		const button = await screen.findByRole('button', { name: 'Open in a window' });
		fireEvent.click(button);
		await waitFor(() => expect(uris(windows)).toEqual([fileLocation('/home/test/notes.txt').uri]));
	});

	it('leaves the Inspector without the button where there is no service', async () => {
		await mount(undefined);
		fireEvent.keyDown(window, { key: 'F11' });
		fireEvent.click(await screen.findByRole('tab', { name: 'Properties' }));
		fireEvent.click(await option('notes.txt'));
		await screen.findAllByText('Kind');
		expect(screen.queryByRole('button', { name: 'Open in a window' })).toBeNull();
	});
});
