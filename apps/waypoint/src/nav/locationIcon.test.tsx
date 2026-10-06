// Verifies the menus that name a location draw its icon in the window's icon theme, as the tab strip does
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import { expectEveryItemHasIcon } from '@liminal-hq/waypoint-chrome/ContextMenu/expectEveryItemHasIcon';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { act, cleanup, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { ConnectionsProvider } from '../connections/ConnectionsContext';
import { FakeConnectionsClient, serverLocation } from '../connections/fakeConnectionsClient';
import { configureSystemIcons } from '../icons/systemIcons';
import { createFakeSystemIconsClient } from '../services/fakeSystemIconsClient';
import { pairTabItems } from '../tabs/pairMenus';
import { plusMenuItems, tabMenuItems } from '../tabs/tabMenuModel';
import { createTree } from '../test/workspaceHarness';
import { historyMenuItems } from './NavButton';

const root = document.documentElement;

const local: Location = { uri: 'file:///home/test/docs', display: '/home/test/docs' };
const server = serverLocation('sftp://me@nas.lan', '/media');

const tab = (id: number, location: Location): TabSnapshot =>
	({
		id,
		location,
		back: [],
		forward: [],
		pinned: false,
		colour: null,
		group: null,
		hints: {},
	}) as unknown as TabSnapshot;

const closed: ClosedTab[] = [
	{ tab: tab(8, local), window: 'main', index: 0 },
	{ tab: tab(9, server), window: 'main', index: 1 },
];

const folders = (scope: HTMLElement) =>
	scope.querySelectorAll('svg[data-group="folder"]:not([data-system])');

async function settle(): Promise<void> {
	await act(async () => {
		for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
	});
}

/** Opens `items` in a real context menu inside the providers a window has, and opens the submenu `open` names. */
async function openMenu(items: MenuItem[], open?: string): Promise<void> {
	const ui: ReactNode = (
		<VfsClientProvider client={createTree()}>
			<ConnectionsProvider client={new FakeConnectionsClient({})}>
				<ContextMenu
					items={items}
					position={{ x: 0, y: 0 }}
					onSelect={() => {}}
					onClose={() => {}}
				/>
			</ConnectionsProvider>
		</VfsClientProvider>
	);
	render(ui);
	await settle();
	if (open) {
		await act(async () => {
			screen.getByRole('menuitem', { name: open }).click();
		});
		await settle();
	}
}

function row(name: string): HTMLElement {
	return screen.getByRole('menuitem', { name });
}

beforeEach(() => {
	delete root.dataset.iconTheme;
});

afterEach(() => {
	cleanup();
	configureSystemIcons(null);
	for (const key of ['iconTheme', 'folderColour', 'theme']) delete root.dataset[key];
});

describe('the Recently Closed rows', () => {
	it('draw the Waypoint folder glyph in the Waypoint set and a server badge for a server tab', async () => {
		root.dataset.iconTheme = 'waypoint';
		await openMenu(tabMenuItems(tab(1, local), closed), 'Recently Closed');
		expect(folders(row('docs'))).toHaveLength(1);
		expect(row('media').querySelector('svg[data-group]')).toBeNull();
		expect(row('media').querySelector('[data-tone] svg')).not.toBeNull();
	});

	it('draw the Portage folder in the chosen colour, in the tab menu and in the + menu', async () => {
		root.dataset.iconTheme = 'portage';
		root.dataset.folderColour = 'red';
		await openMenu(tabMenuItems(tab(1, local), closed), 'Recently Closed');
		const art = row('docs').querySelector('svg[data-group="folder"]');
		expect(art?.getAttribute('data-colour')).toBe('red');
		expect(art?.getAttribute('viewBox')).toBe('0 0 64 64');
		cleanup();
		await openMenu(plusMenuItems(closed), 'Recently Closed');
		const plus = row('docs').querySelector('svg[data-group="folder"]');
		expect(plus?.getAttribute('data-colour')).toBe('red');
	});

	it('draw the system’s folder icon once loaded, with the Waypoint glyph before it', async () => {
		root.dataset.iconTheme = 'system';
		const fake = createFakeSystemIconsClient();
		configureSystemIcons(fake);
		await openMenu(plusMenuItems(closed), 'Recently Closed');
		expect(folders(row('docs'))).toHaveLength(1);
		await act(async () => fake.settle(true));
		const entry = row('docs');
		expect(entry.querySelector('svg[data-system] image')).not.toBeNull();
		expect(folders(entry)).toHaveLength(0);
	});

	it('keep the Waypoint glyph where the system has no folder icons', async () => {
		root.dataset.iconTheme = 'system';
		configureSystemIcons(
			createFakeSystemIconsClient({
				status: { folderIcons: { available: false, reason: 'no theme for folders' } },
			}),
		);
		await openMenu(plusMenuItems(closed), 'Recently Closed');
		expect(folders(row('docs'))).toHaveLength(1);
		expect(row('docs').querySelector('svg[data-system]')).toBeNull();
	});

	it('follow a change of the window’s icon theme while the menu is open', async () => {
		root.dataset.iconTheme = 'waypoint';
		await openMenu(plusMenuItems(closed), 'Recently Closed');
		expect(folders(row('docs'))).toHaveLength(1);
		await act(async () => {
			root.dataset.iconTheme = 'portage';
		});
		await settle();
		expect(row('docs').querySelector('svg[data-colour]')).not.toBeNull();
	});

	it('keep an icon on every item', () => {
		expectEveryItemHasIcon(tabMenuItems(tab(1, local), closed));
		expectEveryItemHasIcon(plusMenuItems(closed));
	});
});

describe('the Back and Forward history rows', () => {
	it('draw the Portage folder, and the server badge for a server location', async () => {
		root.dataset.iconTheme = 'portage';
		await openMenu(historyMenuItems([local, server], 'Back'));
		expect(row('/home/test/docs').querySelector('svg[data-colour]')).not.toBeNull();
		expect(row(server.display).querySelector('[data-tone] svg')).not.toBeNull();
	});

	it('draw the system’s folder icon once loaded', async () => {
		root.dataset.iconTheme = 'system';
		const fake = createFakeSystemIconsClient();
		configureSystemIcons(fake);
		await openMenu(historyMenuItems([local]));
		await act(async () => fake.settle(true));
		expect(row('/home/test/docs').querySelector('svg[data-system] image')).not.toBeNull();
	});
});

describe('the Split With rows', () => {
	it('draw the themed folder icon', async () => {
		root.dataset.iconTheme = 'portage';
		const other = tab(2, { uri: 'file:///home/test/music', display: '/home/test/music' });
		await openMenu(
			pairTabItems(tab(1, local), undefined, [tab(1, local), other], []),
			'Split With',
		);
		expect(row('music').querySelector('svg[data-colour]')).not.toBeNull();
	});
});
