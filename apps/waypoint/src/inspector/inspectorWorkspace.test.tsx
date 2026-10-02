// Verifies the Inspector in the browsing area: F11, the right-click Properties items, the palette command and following the active pane's selection
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { dismissNotice } from '../app/notices';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { CommandPaletteHost } from '../commands/CommandPaletteHost';
import { keyEventInit } from '../commands/shortcuts';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { createFakeThumbnailsClient } from '../thumbnails/fakeThumbnailsClient';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { SettingsProvider } from '../settings/SettingsContext';
import { PlacesClientProvider } from '../sidebar/PlacesClientContext';
import { TabsProvider } from '../tabs/TabsContext';
import { TrashClientProvider } from '../trash/TrashClientContext';
import { Workspace } from '../app/Workspace';
import { DetailsClientProvider } from './DetailsClientContext';
import { fakeDetails } from '../services/fakeDetailsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME, renderWorkspace } from '../test/workspaceHarness';
import { LazyDetails } from './inspectorHarness';

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
const inspector = () => screen.queryByRole('complementary', { name: 'Inspector' });

function details() {
	return new LazyDetails({
		3: { details: fakeDetails({ name: 'notes.txt', owner: 'scott', unavailable: [] }) },
	});
}

describe('the Inspector in the window', () => {
	it('is closed to begin with, and F11 shows and hides it, on the tab last used', async () => {
		await renderWorkspace(createTree(), undefined, undefined, { details: details() });
		await option('notes.txt');
		expect(inspector()).toBeNull();
		fireEvent.keyDown(window, { key: 'F11' });
		const panel = await screen.findByRole('complementary', { name: 'Inspector' });
		expect(within(panel).getByRole('tab', { name: 'Preview' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
		fireEvent.click(within(panel).getByRole('tab', { name: 'Properties' }));
		fireEvent.keyDown(window, { key: 'F11' });
		expect(inspector()).toBeNull();
		fireEvent.keyDown(window, { key: 'F11' });
		const again = await screen.findByRole('complementary', { name: 'Inspector' });
		expect(within(again).getByRole('tab', { name: 'Properties' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
	});

	it('sits at the end of the body, after the panes', async () => {
		await renderWorkspace(createTree(), undefined, undefined, { details: details() });
		fireEvent.keyDown(window, { key: 'F11' });
		const panel = await screen.findByRole('complementary', { name: 'Inspector' });
		const files = document.getElementById('wp-tabpanel')!;
		expect(files.compareDocumentPosition(panel) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
		// The files sit in the content column beside the sidebar; the Inspector is the body row's last child.
		expect(panel.parentElement).toBe(files.parentElement!.parentElement);
	});

	it('follows the selection of the active pane', async () => {
		await renderWorkspace(createTree(), undefined, undefined, { details: details() });
		fireEvent.keyDown(window, { key: 'F11' });
		fireEvent.click(await screen.findByRole('tab', { name: 'Properties' }));
		const panel = await screen.findByRole('complementary', { name: 'Inspector' });
		expect(within(panel).getByText('Contains')).toBeInTheDocument();
		fireEvent.click(await option('notes.txt'));
		await waitFor(() => expect(within(panel).getByText('Owner')).toBeInTheDocument());
		expect(within(panel).getAllByText('notes.txt').length).toBeGreaterThan(0);
	});

	it('opens on the Properties tab from the right-click menu of an item', async () => {
		await renderWorkspace(createTree(), undefined, undefined, { details: details() });
		fireEvent.contextMenu(await option('notes.txt'));
		const menu = await screen.findByRole('menu', { name: 'Item actions' });
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Properties' }));
		const panel = await screen.findByRole('complementary', { name: 'Inspector' });
		expect(within(panel).getByRole('tab', { name: 'Properties' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
		await waitFor(() => expect(within(panel).getByText('Owner')).toBeInTheDocument());
	});

	it('opens on the Properties tab from the menu of the empty space, for the folder', async () => {
		await renderWorkspace(createTree(), undefined, undefined, { details: details() });
		await option('notes.txt');
		const list = await screen.findByRole('listbox');
		fireEvent.contextMenu(list);
		const menu = await screen.findByRole('menu');
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Properties' }));
		const panel = await screen.findByRole('complementary', { name: 'Inspector' });
		expect(within(panel).getByRole('tab', { name: 'Properties' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
		expect(within(panel).getByText('Contains')).toBeInTheDocument();
	});

	it('starts the existing rename from the name’s button, and leaves editing to it', async () => {
		await renderWorkspace(createTree(), undefined, undefined, {
			details: details(),
			ops: createFakeOpsClient(),
		});
		fireEvent.keyDown(window, { key: 'F11' });
		fireEvent.click(await screen.findByRole('tab', { name: 'Properties' }));
		fireEvent.click(await option('notes.txt'));
		fireEvent.click(await screen.findByRole('button', { name: 'Rename…' }));
		const field = await screen.findByRole('textbox');
		expect(field).toHaveValue('notes.txt');
	});

	it('asks for the large thumbnail of a picture while it loads, and not for a folder’s', async () => {
		const thumbs = createFakeThumbnailsClient();
		await renderWorkspace(createTree(), undefined, undefined, {
			details: details(),
			thumbnails: thumbs,
		});
		fireEvent.keyDown(window, { key: 'F11' });
		await screen.findByRole('complementary', { name: 'Inspector' });
		fireEvent.click(await option('docs'));
		fireEvent.click(await option('photo.jpg'));
		await waitFor(() => expect(thumbs.batches.some((batch) => batch.size === 'large')).toBe(true));
		const large = thumbs.batches.filter((batch) => batch.size === 'large').flatMap((b) => b.keys);
		expect(large).toHaveLength(1);
		expect(large[0]).toMatch(/^4:/);
	});

	it('runs Inspector and Properties from the command palette', async () => {
		const vfs = createTree();
		const tabs = new FakeTabsApi();
		await tabs.openTab(HOME);
		const bridge = createCommandBridge();
		render(
			<VfsClientProvider client={vfs}>
				<PlacesClientProvider client={new FakePlacesClient({ places: fakePlaces('/home/test') })}>
					<TrashClientProvider client={undefined}>
						<SettingsProvider client={createFakeSettingsClient()}>
							<DetailsClientProvider client={details()}>
								<TabsProvider api={tabs} home={HOME}>
									<CommandBridgeProvider value={bridge}>
										<Workspace />
										<CommandPaletteHost />
									</CommandBridgeProvider>
								</TabsProvider>
							</DetailsClientProvider>
						</SettingsProvider>
					</TrashClientProvider>
				</PlacesClientProvider>
			</VfsClientProvider>,
		);
		await option('notes.txt');
		fireEvent.keyDown(window, keyEventInit('Ctrl+Shift+P'));
		const input = await screen.findByRole('combobox');
		await userEvent.type(input, 'propert');
		await userEvent.keyboard('{Enter}');
		const panel = await screen.findByRole('complementary', { name: 'Inspector' });
		expect(within(panel).getByRole('tab', { name: 'Properties' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
		expect(bridge.store.getState().facts.inspectorOpen).toBe(true);
		// F11 hides it again, and the fact follows.
		fireEvent.keyDown(window, { key: 'F11' });
		await waitFor(() => expect(inspector()).toBeNull());
		expect(bridge.store.getState().facts.inspectorOpen).toBe(false);
	});
});
