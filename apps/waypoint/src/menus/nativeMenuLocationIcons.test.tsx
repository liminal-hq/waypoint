// Verifies the native menus that name a location (Recently Closed, Back and Forward history, Split With) give each row the bitmap of its themed folder
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { ConnectionsProvider } from '../connections/ConnectionsContext';
import { FakeConnectionsClient, serverLocation } from '../connections/fakeConnectionsClient';
import { configureSystemIcons } from '../icons/systemIcons';
import { historyMenuItems } from '../nav/NavButton';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { createFakeSystemIconsClient } from '../services/fakeSystemIconsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { SettingsProvider, useSettingsReady } from '../settings/SettingsContext';
import { pairTabItems } from '../tabs/pairMenus';
import { plusMenuItems, tabMenuItems } from '../tabs/tabMenuModel';
import { createTree } from '../test/workspaceHarness';
import { createFakeNativeMenuClient } from './fakeNativeMenuClient';
import { HostedContextMenu } from './HostedContextMenu';
import type { RasteriserEnvironment } from './menuIconRaster';
import { MenuIconStageHost } from './MenuIconStageHost';
import { NativeMenuProvider } from './NativeMenuContext';
import type { NativeMenuItem } from './nativeMenuClient';

const root = document.documentElement;

const local: Location = { uri: 'file:///home/test/docs', display: '/home/test/docs' };
const music: Location = { uri: 'file:///home/test/music', display: '/home/test/music' };
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

const environment: RasteriserEnvironment = {
	scale: () => 1,
	draw: async (_markup, px) => new Array<number>(px * px * 4).fill(7),
	drawImage: async (_url, px) => new Array<number>(px * px * 4).fill(9),
};

function Gate({ children }: { children: React.ReactNode }) {
	return useSettingsReady() ? <>{children}</> : null;
}

/** The rows of a native menu, submenus opened out, by label. */
function rowsOf(items: NativeMenuItem[]): Map<string, NativeMenuItem> {
	const rows = new Map<string, NativeMenuItem>();
	for (const item of items) {
		if (item.kind === 'submenu') {
			for (const [label, row] of rowsOf(item.items)) rows.set(label, row);
		} else if (item.kind !== 'separator') {
			rows.set(item.label, item);
		}
	}
	return rows;
}

async function openNatively(items: MenuItem[]) {
	const native = createFakeNativeMenuClient();
	const settings = createFakeSettingsClient({
		...DEFAULT_SETTINGS,
		experimental: { ...DEFAULT_SETTINGS.experimental, nativeContextMenus: true },
	});
	render(
		<SettingsProvider client={settings}>
			<NativeMenuProvider client={native} platform="linux" environment={environment}>
				<VfsClientProvider client={createTree()}>
					<ConnectionsProvider client={new FakeConnectionsClient({})}>
						<MenuIconStageHost />
						<Gate>
							<HostedContextMenu
								items={items}
								position={{ x: 10, y: 10 }}
								ariaLabel="Menu"
								onSelect={() => {}}
								onClose={() => {}}
							/>
						</Gate>
					</ConnectionsProvider>
				</VfsClientProvider>
			</NativeMenuProvider>
		</SettingsProvider>,
	);
	await waitFor(() => expect(native.calls).toHaveLength(1));
	return rowsOf(native.calls[0]?.items ?? []);
}

const pictured = (row: NativeMenuItem | undefined) =>
	row && row.kind === 'action' ? row.icon : undefined;

beforeEach(() => {
	delete root.dataset.iconTheme;
});

afterEach(() => {
	cleanup();
	configureSystemIcons(null);
	for (const key of ['iconTheme', 'folderColour', 'theme']) delete root.dataset[key];
	vi.restoreAllMocks();
});

describe.each(['waypoint', 'portage'])('with native menus on, in the %s icon set', (theme) => {
	beforeEach(() => {
		root.dataset.iconTheme = theme;
	});

	it('gives the Recently Closed rows, in the tab menu and the + menu, a bitmap each', async () => {
		for (const items of [tabMenuItems(tab(1, local), closed), plusMenuItems(closed)]) {
			const rows = await openNatively(items);
			for (const label of ['docs', 'media']) {
				expect(pictured(rows.get(label)), label).toMatchObject({ width: 16, height: 16 });
			}
			cleanup();
		}
	});

	it('gives the Back and Forward history rows a bitmap each', async () => {
		const rows = await openNatively(historyMenuItems([local, server], 'Back'));
		expect(pictured(rows.get(local.display))).toMatchObject({ width: 16, height: 16 });
		expect(pictured(rows.get(server.display))).toMatchObject({ width: 16, height: 16 });
	});

	it('gives the Split With rows a bitmap each', async () => {
		const rows = await openNatively(
			pairTabItems(tab(1, local), undefined, [tab(1, local), tab(2, music)], []),
		);
		expect(pictured(rows.get('music'))).toMatchObject({ width: 16, height: 16 });
	});
});

describe('with native menus on, in the System icon set', () => {
	it('gives the rows the system’s folder picture once it has loaded', async () => {
		root.dataset.iconTheme = 'system';
		const fake = createFakeSystemIconsClient();
		configureSystemIcons(fake);
		const opened = openNatively(historyMenuItems([local, music]));
		await act(async () => {
			for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
		});
		await act(async () => fake.settle(true));
		const rows = await opened;
		for (const row of [local, music]) {
			expect(pictured(rows.get(row.display))?.rgba[0], row.display).toBe(9);
		}
		expect(fake.probed).toHaveLength(1);
		expect(screen.queryByRole('menu')).toBeNull();
	});
});
