// Verifies the menus meant for the system's menu convert, and that the Trash menu runs the same action through one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { isValidElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { backgroundMenuItems } from '../browse/BackgroundContextMenu';
import { sidebarMenuItems } from '../sidebar/SidebarMenu';
import { SettingsProvider, useSettingsReady } from '../settings/SettingsContext';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { TabColourSwatch } from '../tabs/TabColourSwatch';
import { tabMenuItems } from '../tabs/tabMenuModel';
import { TrashEntryMenu } from '../trash/TrashEntryMenu';
import { createFakeNativeMenuClient } from './fakeNativeMenuClient';
import { toNativeMenu } from './nativeMenu';
import { NativeMenuProvider } from './NativeMenuContext';

afterEach(cleanup);

function convertible(items: Parameters<typeof toNativeMenu>[0]) {
	const result = toNativeMenu(items, () => null);
	expect(result.ok).toBe(true);
	return result;
}

describe('the menus meant for the system’s menu', () => {
	it('convert: the file list background, with and without a sort', () => {
		convertible(backgroundMenuItems(undefined, false));
		convertible(
			backgroundMenuItems(
				{ key: 'name', descending: false, directoriesFirst: true, groupBy: 'none' },
				true,
			),
		);
		convertible(
			backgroundMenuItems(
				{ key: 'deleted', descending: true, directoriesFirst: false, groupBy: 'none' },
				false,
				{ trash: { count: 2 } },
			),
		);
	});

	it('convert: every kind of sidebar item', () => {
		for (const kind of ['place', 'trash', 'favourite', 'folder'] as const) {
			convertible(
				sidebarMenuItems(kind, {
					favouritePosition: { index: 1, count: 3 },
					pinned: false,
					canRename: true,
					trash: { count: 1, available: true },
				}),
			);
		}
	});

	it('convert: a tab’s menu, whose colour swatches become icon items when they have a picture', () => {
		const tab = {
			id: 't1',
			title: 'Home',
			pinned: false,
			colour: 'blue',
			group: null,
		} as unknown as Parameters<typeof tabMenuItems>[0];
		const result = toNativeMenu(tabMenuItems(tab, [], [], []), (icon) =>
			isValidElement(icon) && icon.type === TabColourSwatch
				? { width: 1, height: 1, rgba: [0, 0, 0, 255], marksCheck: true }
				: null,
		);
		expect(result.ok).toBe(true);
		if (!result.ok) return;
		expect(result.selectable.has('colour:blue')).toBe(true);
		const items = result.items.flatMap((item) => (item.kind === 'submenu' ? item.items : []));
		expect(items.find((item) => 'id' in item && item.id === 'colour:blue')).toMatchObject({
			kind: 'action',
			icon: { width: 1 },
		});
		expect(items.find((item) => 'id' in item && item.id === 'colour:none')).toMatchObject({
			kind: 'checkbox',
			checked: false,
		});
	});
});

function Gate({ children }: { children: React.ReactNode }) {
	return useSettingsReady() ? <>{children}</> : null;
}

describe('the Trash item menu through the system’s menu', () => {
	it('restores through the same handler the page’s menu runs', async () => {
		const native = createFakeNativeMenuClient();
		native.answer('restore');
		const onRestore = vi.fn();
		const onDelete = vi.fn();
		const onClose = vi.fn();
		render(
			<SettingsProvider
				client={createFakeSettingsClient({
					...DEFAULT_SETTINGS,
					experimental: { ...DEFAULT_SETTINGS.experimental, nativeContextMenus: true },
				})}
			>
				<NativeMenuProvider client={native} platform="windows" rasterise={async () => null}>
					<Gate>
						<TrashEntryMenu
							position={{ x: 5, y: 6 }}
							keyboard={false}
							onRestore={onRestore}
							onDelete={onDelete}
							onClose={onClose}
						/>
					</Gate>
				</NativeMenuProvider>
			</SettingsProvider>,
		);
		await waitFor(() => expect(onRestore).toHaveBeenCalledTimes(1));
		expect(onDelete).not.toHaveBeenCalled();
		expect(
			native.calls[0]?.items.map((item) => (item.kind === 'action' ? item.id : item.kind)),
		).toEqual(['restore', 'separator', 'delete']);
		expect(screen.queryByRole('menu')).toBeNull();
	});
});
