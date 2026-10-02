// Verifies every context menu item has an icon, in every state its builder can produce
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expectEveryItemHasIcon } from '@liminal-hq/waypoint-chrome/ContextMenu/expectEveryItemHasIcon';
import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Group } from '@liminal-hq/waypoint-protocol/generated/Group';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import type { Workspace } from '@liminal-hq/waypoint-protocol/generated/Workspace';
import { describe, expect, it } from 'vitest';
import { backgroundMenuItems } from '../browse/BackgroundContextMenu';
import { entryMenuItems } from '../browse/EntryContextMenu';
import { historyMenuItems } from '../nav/NavButton';
import { shelfItemMenuItems, shelfPanelMenuItems } from '../shelf/shelfMenu';
import { sidebarMenuItems } from '../sidebar/SidebarMenu';
import { trashEntryMenuItems } from '../trash/TrashEntryMenu';
import { workspaceMenuItems } from '../sidebar/WorkspaceMenu';
import { groupMenuItems } from '../tabs/GroupMenu';
import { pairJointItems, pairTabItems } from '../tabs/pairMenus';
import { plusMenuItems, tabMenuItems } from '../tabs/tabMenuModel';

const place: Location = { uri: 'file:///home/a', display: '/home/a' };

const tab = (id: number, extra: Partial<TabSnapshot> = {}): TabSnapshot =>
	({
		id,
		location: place,
		back: [],
		forward: [],
		pinned: false,
		colour: null,
		group: null,
		hints: {},
		...extra,
	}) as unknown as TabSnapshot;

const group: Group = { id: 1, name: 'Work', colour: 'blue', collapsed: false };
const plain: Group = { id: 2, name: 'Home', colour: null, collapsed: true };
const windows: WindowSummary[] = [{ label: 'w2', title: 'Docs', tabCount: 2, active: false }];
const closed: ClosedTab[] = [{ tab: tab(9), window: 'main', index: 0 }];
const pair: Pair = {
	id: 1,
	panes: [1, 2],
	layout: 'sideBySide',
	sizes: [500, 500],
	origin: { kind: 'joined' },
};
const entry = (kind: 'directory' | 'file'): Entry =>
	({ id: 'e', name: 'x', kind, linkTarget: null, group: 'folder', size: null }) as unknown as Entry;
const workspace = { id: 1, name: 'W', locations: [place] } as unknown as Workspace;

describe('context menu icons', () => {
	it('gives the entry menu an icon on every item, for a folder and for a file', () => {
		expectEveryItemHasIcon(entryMenuItems(entry('directory')));
		expectEveryItemHasIcon(entryMenuItems(entry('file')));
	});

	it('gives the empty-space menu an icon on every item, with and without a listing', () => {
		expectEveryItemHasIcon(
			backgroundMenuItems(
				{ key: 'name', descending: false, directoriesFirst: true, groupBy: 'none' },
				true,
			),
		);
		expectEveryItemHasIcon(backgroundMenuItems(undefined, false));
	});

	it('gives the Trash menus an icon on every item', () => {
		expectEveryItemHasIcon(trashEntryMenuItems());
		const sort = {
			key: 'deleted' as const,
			descending: false,
			directoriesFirst: true,
			groupBy: 'none' as const,
		};
		expectEveryItemHasIcon(backgroundMenuItems(sort, false, { trash: { count: 2 } }));
		expectEveryItemHasIcon(backgroundMenuItems(undefined, false, { trash: { count: 0 } }));
	});

	it('gives the tab menu an icon on every item, grouped, ungrouped and with other windows', () => {
		expectEveryItemHasIcon(tabMenuItems(tab(1), closed, [group, plain], windows));
		expectEveryItemHasIcon(tabMenuItems(tab(1, { pinned: true, group: 1 }), [], [group], []));
	});

	it('gives the tab menu’s split section an icon on every item', () => {
		expectEveryItemHasIcon(pairTabItems(tab(1), undefined, [tab(1), tab(2)], []));
		expectEveryItemHasIcon(pairTabItems(tab(1), pair, [tab(1), tab(2)], [pair]));
	});

	it('gives the + menu an icon on every item', () => {
		expectEveryItemHasIcon(plusMenuItems(closed));
		expectEveryItemHasIcon(plusMenuItems([]));
	});

	it('gives the group menu an icon on every item', () => {
		expectEveryItemHasIcon(groupMenuItems(group, { pinned: false, hasOthers: true }, windows));
		expectEveryItemHasIcon(groupMenuItems(plain, { pinned: true, hasOthers: false }));
	});

	it('gives the pair joint menu an icon on every item', () => {
		expectEveryItemHasIcon(pairJointItems(pair, [tab(1), tab(2)], [], windows));
		expectEveryItemHasIcon(pairJointItems(pair, [tab(1, { pinned: true }), tab(2)]));
	});

	it('gives each kind of sidebar menu an icon on every item', () => {
		const state = { favouritePosition: { index: 1, count: 3 }, pinned: false, canRename: true };
		for (const kind of ['place', 'trash', 'favourite', 'folder'] as const) {
			expectEveryItemHasIcon(sidebarMenuItems(kind, state));
		}
		expectEveryItemHasIcon(
			sidebarMenuItems('trash', { ...state, trash: { count: 3, available: true } }),
		);
		expectEveryItemHasIcon(sidebarMenuItems('favourite', { ...state, canRename: false }));
	});

	it('gives the workspace menu an icon on every item', () => {
		expectEveryItemHasIcon(workspaceMenuItems(workspace));
	});

	it('gives the history menu an icon on every item', () => {
		expectEveryItemHasIcon(historyMenuItems([place, place], 'Recent folders'));
	});

	it('gives the Shelf’s menus an icon on every item', () => {
		expectEveryItemHasIcon(shelfItemMenuItems(1, true));
		expectEveryItemHasIcon(shelfItemMenuItems(3, false));
		expectEveryItemHasIcon(shelfPanelMenuItems(2));
	});

	it('fails, naming the item, when one has no icon', () => {
		expect(() => expectEveryItemHasIcon([{ type: 'action', id: 'bare', label: 'Bare' }])).toThrow(
			/bare/,
		);
	});
});
