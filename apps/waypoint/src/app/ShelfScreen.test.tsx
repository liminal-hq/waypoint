// Verifies the Shelf window: the same items and keyboard model as the dock, kept level with every other window through the one session, with Dock and Always on Top
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import type { WindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/windowControls';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { FakeShelfWindowClient } from '../services/fakeShelfWindowClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore, SHELF_LABEL } from '../services/fakeTabsStore';
import { fileLocation } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME } from '../test/workspaceHarness';
import { dismissNotice } from './notices';
import { ShelfScreen, type ShelfScreenServices } from './ShelfScreen';
import { WindowCapabilitiesProvider } from './windowCapabilities';

vi.mock('@liminal-hq/plugin-window-manager', () => ({
	getCapabilities: vi.fn().mockResolvedValue({
		session: 'x11',
		alwaysOnTop: true,
		systemWindowMenu: false,
	}),
	getAlwaysOnTop: vi.fn().mockResolvedValue(false),
	onAlwaysOnTopChanged: vi.fn().mockResolvedValue(() => {}),
	showSystemWindowMenu: vi.fn(),
}));
vi.mock('../services/titlebarPreferences', () => ({ useTitlebarPreferences: () => null }));

const NOTES = fileLocation('/home/test/notes.txt');
const PHOTO = fileLocation('/home/test/photo.jpg');
const REPORT = fileLocation('/home/test/docs/report.pdf');

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
});

async function mount(options: { onTop?: boolean } = {}) {
	const store = new FakeTabsStore();
	const main = new FakeTabsApi(store, 'main-1');
	await main.openTab(HOME);
	await main.addToShelf([NOTES, PHOTO, REPORT]);
	await main.setShelfUndocked(true);
	if (options.onTop) await new FakeTabsApi(store, SHELF_LABEL).setShelfOnTop(true);
	const tabsApi = new FakeTabsApi(store, SHELF_LABEL);
	const windowClient = new FakeShelfWindowClient();
	const services: ShelfScreenServices = {
		home: HOME,
		vfs: createTree(),
		tabsApi,
		ops: createFakeOpsClient(),
		windowClient,
	};
	const controls: WindowControls = {
		minimize: vi.fn(),
		toggleMaximize: vi.fn(),
		close: vi.fn(),
		setAlwaysOnTop: vi.fn(),
		isAlwaysOnTop: async () => false,
		isMaximized: async () => false,
		onMaximizedChange: () => () => {},
	};
	render(
		<WindowCapabilitiesProvider>
			<WindowChromeProvider controls={controls}>
				<ShelfScreen services={services} />
			</WindowChromeProvider>
		</WindowCapabilitiesProvider>,
	);
	await screen.findByRole('tree', { name: 'Shelf items' });
	return { store, main, controls, windowClient };
}

const items = () =>
	[...document.querySelectorAll<HTMLElement>('[role="treeitem"][aria-level="2"]')].map((row) =>
		row.querySelector('[class*="name"]')?.textContent?.trim(),
	);

describe('the window', () => {
	it('shows the shared Shelf the way the dock does: grouped by folder, with names a screen reader can use', async () => {
		await mount();
		expect(screen.getByRole('complementary', { name: 'Shelf' })).toHaveAttribute(
			'data-layout',
			'window',
		);
		// The newest item comes first, under the folder it was taken from.
		expect(items()).toEqual(['report.pdf', 'photo.jpg', 'notes.txt']);
		expect(screen.getByRole('tree', { name: 'Shelf items' })).toHaveAttribute(
			'aria-multiselectable',
			'true',
		);
		expect(
			screen.getByRole('button', { name: 'Remove notes.txt from the Shelf' }),
		).toBeInTheDocument();
		expect(document.querySelectorAll('[role="treeitem"][aria-level="1"]')).toHaveLength(2);
		expect(screen.getAllByText('Waypoint — Shelf').length).toBeGreaterThan(0);
	});

	it('follows a change made in a main window, and a main window follows a change made in it', async () => {
		const { main, store } = await mount();
		await act(async () => main.addToShelf([fileLocation('/home/test/music/song.mp3')]));
		await waitFor(() => expect(items()).toContain('song.mp3'));
		fireEvent.click(screen.getByRole('button', { name: 'Remove photo.jpg from the Shelf' }));
		await waitFor(() => expect(items()).not.toContain('photo.jpg'));
		expect(store.snapshot('main-1').shelf.map((i) => i.name)).not.toContain('photo.jpg');
	});

	it('has no divider and no close button of the dock, and a Dock button in their place', async () => {
		await mount();
		expect(screen.queryByRole('separator', { name: 'Resize the Shelf' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Hide the Shelf' })).toBeNull();
		expect(
			screen.getByRole('button', { name: 'Dock the Shelf back into the window' }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole('button', { name: 'Undock the Shelf into its own window' }),
		).toBeNull();
	});
});

describe('the keyboard and the menu', () => {
	it('selects, moves and removes with the keys the dock uses', async () => {
		const { store } = await mount();
		const tree = screen.getByRole('tree', { name: 'Shelf items' });
		tree.focus();
		fireEvent.keyDown(tree, { key: 'ArrowDown' });
		fireEvent.keyDown(tree, { key: 'ArrowRight' });
		fireEvent.keyDown(tree, { key: ' ' });
		const selected = within(tree).getAllByRole('treeitem', { selected: true });
		expect(selected).toHaveLength(1);
		fireEvent.keyDown(tree, { key: 'Delete' });
		await waitFor(() => expect(store.snapshot('main-1').shelf).toHaveLength(2));
		expect(items()).toHaveLength(2);
	});

	it('opens the per-item menu on a right-click', async () => {
		await mount();
		const row = document.querySelector<HTMLElement>('[role="treeitem"][aria-level="2"]')!;
		fireEvent.contextMenu(row, { clientX: 20, clientY: 20 });
		const menu = await screen.findByRole('menu', { name: 'Shelf item' });
		const labels = within(menu)
			.getAllByRole('menuitem')
			.map((item) => item.textContent ?? '');
		for (const wanted of ['Open', 'Reveal in Folder', 'Copy Path', 'Remove from Shelf']) {
			expect(
				labels.some((label) => label.startsWith(wanted)),
				wanted,
			).toBe(true);
		}
	});

	it('hides the window on Ctrl+B instead of closing a dock', async () => {
		const { windowClient } = await mount();
		fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
		await waitFor(() => expect(windowClient.calls).toContain('toggle'));
		expect(screen.getByRole('complementary', { name: 'Shelf' })).toBeInTheDocument();
	});
});

describe('docking back and staying on top', () => {
	it('Dock puts the Shelf back in the main windows through the session', async () => {
		const { store } = await mount();
		fireEvent.click(screen.getByRole('button', { name: 'Dock the Shelf back into the window' }));
		await waitFor(() => expect(store.snapshot('main-1').shelfWindow.undocked).toBe(false));
	});

	it('remembers the window staying on top, from the title bar’s pin', async () => {
		const { store, controls } = await mount();
		fireEvent.click(await screen.findByRole('button', { name: 'Always on Top' }));
		await waitFor(() => expect(store.snapshot('main-1').shelfWindow.onTop).toBe(true));
		expect(controls.setAlwaysOnTop).toHaveBeenCalledWith(true);
	});
});
