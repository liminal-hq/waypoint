// Verifies a main window around the undocked Shelf: its dock gives way to the Shelf window, the toggles and commands act on that window, and docking brings the dock back
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dismissNotice } from '../app/notices';
import { Workspace } from '../app/Workspace';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { CommandPaletteHost } from '../commands/CommandPaletteHost';
import { keyEventInit } from '../commands/shortcuts';
import { FakeShelfWindowClient } from '../services/fakeShelfWindowClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { fileLocation } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { SettingsProvider } from '../settings/SettingsContext';
import { PlacesClientProvider } from '../sidebar/PlacesClientContext';
import { TabsProvider } from '../tabs/TabsContext';
import { createTree, HOME, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
	localStorage.clear();
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
});

async function mount() {
	const store = new FakeTabsStore();
	const tabs = new FakeTabsApi(store, 'main-1');
	await tabs.openTab(HOME);
	const windowClient = new FakeShelfWindowClient();
	await renderWorkspace(createTree(), tabs, undefined, { shelfWindow: windowClient });
	await tabs.addToShelf([fileLocation('/home/test/notes.txt')]);
	return { store, tabs, windowClient };
}

const dock = () => screen.queryByRole('complementary', { name: 'Shelf' });
const toggle = () => screen.getByRole('button', { name: 'Shelf' });
const openDock = async () => {
	fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
	return await screen.findByRole('complementary', { name: 'Shelf' });
};

describe('undocking from the dock', () => {
	it('has a button on the dock header that moves the Shelf into its own window', async () => {
		const { store } = await mount();
		await openDock();
		fireEvent.click(screen.getByRole('button', { name: 'Undock the Shelf into its own window' }));
		await waitFor(() => expect(store.snapshot('main-1').shelfWindow.undocked).toBe(true));
		await waitFor(() => expect(dock()).toBeNull());
	});

	it('leaves the Shelf docked, with a notice, when the window cannot be made', async () => {
		const store = new FakeTabsStore({
			createWindow: () => {
				throw new Error('no windows here');
			},
		});
		const tabs = new FakeTabsApi(store, 'main-1');
		await tabs.openTab(HOME);
		await renderWorkspace(createTree(), tabs, undefined, {
			shelfWindow: new FakeShelfWindowClient(),
		});
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		await openDock();
		fireEvent.click(screen.getByRole('button', { name: 'Undock the Shelf into its own window' }));
		await waitFor(() =>
			expect(screen.getByText('Could not open the Shelf in its own window')).toBeInTheDocument(),
		);
		expect(store.snapshot('main-1').shelfWindow.undocked).toBe(false);
		expect(dock()).not.toBeNull();
	});
});

describe('while the Shelf is its own window', () => {
	it('hides the dock, even one that was open, and every main window does', async () => {
		const { tabs } = await mount();
		await openDock();
		await act(async () => tabs.setShelfUndocked(true));
		await waitFor(() => expect(dock()).toBeNull());
	});

	it('has Ctrl+B raise or hide the Shelf window instead of opening a dock', async () => {
		const { tabs, windowClient } = await mount();
		await act(async () => tabs.setShelfUndocked(true));
		fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
		await waitFor(() => expect(windowClient.calls).toContain('toggle'));
		expect(dock()).toBeNull();
	});

	it('has the status bar button raise or hide the window, and be pressed while it is on screen', async () => {
		const { tabs, windowClient } = await mount();
		await act(async () => tabs.setShelfUndocked(true));
		await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'true'));
		fireEvent.click(toggle());
		await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'false'));
		expect(windowClient.shown).toBe(false);
		fireEvent.click(toggle());
		await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'true'));
		expect(dock()).toBeNull();
	});

	it('follows the window being hidden by somebody else', async () => {
		const { tabs, windowClient } = await mount();
		await act(async () => tabs.setShelfUndocked(true));
		await waitFor(() => expect(toggle()).toHaveAttribute('aria-pressed', 'true'));
		act(() => windowClient.show(false));
		expect(toggle()).toHaveAttribute('aria-pressed', 'false');
	});
});

describe('docking again', () => {
	it('brings the dock back open, in every main window, however the Shelf window went', async () => {
		const { store, tabs } = await mount();
		await act(async () => tabs.setShelfUndocked(true));
		await waitFor(() => expect(dock()).toBeNull());
		// The person closes the Shelf window.
		act(() => store.destroyWindow('shelf'));
		expect(await screen.findByRole('complementary', { name: 'Shelf' })).toBeInTheDocument();
	});
});

describe('the commands', () => {
	async function mountWithPalette() {
		const store = new FakeTabsStore();
		const tabs = new FakeTabsApi(store, 'main-1');
		await tabs.openTab(HOME);
		const bridge = createCommandBridge();
		const client = new FakeShelfWindowClient();
		render(
			<VfsClientProvider client={createTree()}>
				<PlacesClientProvider client={new FakePlacesClient({ places: fakePlaces('/home/test') })}>
					<SettingsProvider client={createFakeSettingsClient()}>
						<TabsProvider api={tabs} home={HOME}>
							<CommandBridgeProvider value={bridge}>
								<Workspace shelfWindow={client} />
								<CommandPaletteHost />
							</CommandBridgeProvider>
						</TabsProvider>
					</SettingsProvider>
				</PlacesClientProvider>
			</VfsClientProvider>,
		);
		return { store, bridge, client };
	}
	const search = async (query: string) => {
		fireEvent.keyDown(window, keyEventInit('Ctrl+Shift+P'));
		const input = await screen.findByRole('combobox');
		fireEvent.change(input, { target: { value: query } });
		return input;
	};

	it('offers Undock Shelf in the palette while docked, and runs it', async () => {
		const { store } = await mountWithPalette();
		const input = await search('undock shelf');
		expect(await screen.findByRole('option', { name: /Undock Shelf/ })).toBeInTheDocument();
		expect(screen.queryByRole('option', { name: /^Dock Shelf/ })).toBeNull();
		fireEvent.keyDown(input, { key: 'Enter' });
		await waitFor(() => expect(store.snapshot('main-1').shelfWindow.undocked).toBe(true));
	});

	it('offers Dock Shelf instead while the Shelf is its own window, and runs it', async () => {
		const { store, bridge } = await mountWithPalette();
		await act(async () => store.dispatch('main-1', { kind: 'setShelfUndocked', undocked: true }));
		await waitFor(() => expect(bridge.store.getState().facts.shelfUndocked).toBe(true));
		const input = await search('dock shelf');
		expect(await screen.findByRole('option', { name: /^Dock Shelf/ })).toBeInTheDocument();
		expect(screen.queryByRole('option', { name: /Undock Shelf/ })).toBeNull();
		fireEvent.keyDown(input, { key: 'Enter' });
		await waitFor(() => expect(store.snapshot('main-1').shelfWindow.undocked).toBe(false));
	});

	it('has Focus Shelf raise the window while it is undocked', async () => {
		const { store, bridge, client } = await mountWithPalette();
		await act(async () => store.dispatch('main-1', { kind: 'setShelfUndocked', undocked: true }));
		await waitFor(() => expect(bridge.store.getState().facts.shelfUndocked).toBe(true));
		expect(bridge.store.getState().facts.shelfOpen).toBe(true);
		act(() => bridge.store.getState().actions.focusShelf());
		expect(client.calls).toContain('raise');
		act(() => client.show(false));
		await waitFor(() => expect(bridge.store.getState().facts.shelfOpen).toBe(false));
	});
});
