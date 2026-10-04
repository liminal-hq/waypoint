// The items of the tab menu and the + button menu, and what choosing each one does
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { MenuItem, SelectableMenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Group } from '@liminal-hq/waypoint-protocol/generated/Group';
import { CloseSmallIcon, FolderTabIcon, HomeIcon, PinIcon, PlusIcon } from '../icons/AppIcons';
import {
	CircleIcon,
	CloseOthersIcon,
	CloseToRightIcon,
	CopyIcon,
	GroupIcon,
	HistoryIcon,
	NewTabIcon,
	PaletteIcon,
	PinOffIcon,
	UndoIcon,
	UngroupIcon,
	WindowIcon,
} from '../icons/MenuIcons';
import { t, tf } from '../i18n/messages';
import type { GroupActions } from './groupActions';
import { colourMessageId, TAB_COLOURS } from './tabColours';
import { TabColourSwatch } from './TabColourSwatch';
import type { TabActions } from './tabActions';
import type { TabExtras } from './tabExtras';
import { locationLabel } from './tabTitle';
import { createElement } from 'react';
import { ServerIcon } from '../connections/ConnectionIcons';
import { openConnectDialog } from '../connections/connectStore';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { runWindowMoveItem, windowMoveItems } from './windowMoveMenu';
import type { WindowActions } from './windowActions';

const REOPEN_SHORTCUT = 'Ctrl+Shift+T';
const CLOSED_PREFIX = 'closed:';
const COLOUR_PREFIX = 'colour:';
const GROUP_PREFIX = 'group:';

/** The Reopen Closed Tab action and the Recently Closed submenu, shared by both menus. */
export function closedTabItems(closed: readonly ClosedTab[]): MenuItem[] {
	return [
		{
			type: 'action',
			id: 'reopen',
			label: t('tabs.menu.reopen'),
			shortcut: REOPEN_SHORTCUT,
			icon: createElement(UndoIcon),
			disabled: closed.length === 0,
		},
		{
			type: 'submenu',
			id: 'recentlyClosed',
			label: t('tabs.menu.recentlyClosed'),
			icon: createElement(HistoryIcon),
			disabled: closed.length === 0,
			items: closed.map((entry) => ({
				type: 'action' as const,
				id: `${CLOSED_PREFIX}${entry.tab.id}`,
				label: locationLabel(entry.tab.location),
				icon: createElement(FolderTabIcon),
			})),
		},
	];
}

/**
 * The tab menu. It is built from sections so that later slices add theirs (Split With, Add to
 * Group, Move to Window) between the existing ones without reworking the rest.
 */
export function tabMenuItems(
	tab: TabSnapshot,
	closed: readonly ClosedTab[],
	groups: readonly Group[] = [],
	others: readonly WindowSummary[] = [],
): MenuItem[] {
	const own = groups.find((group) => group.id === tab.group);
	const pinWording = (pinned: boolean) =>
		own
			? // Pinning a grouped tab pins its whole group, so the item says whose.
				tf(pinned ? 'tabs.menu.unpinGroup' : 'tabs.menu.pinGroup', { name: own.name })
			: t(pinned ? 'tabs.menu.unpin' : 'tabs.menu.pin');
	return [
		{
			type: 'action',
			id: tab.pinned ? 'unpin' : 'pin',
			label: pinWording(tab.pinned),
			icon: createElement(tab.pinned ? PinOffIcon : PinIcon),
		},
		{
			type: 'submenu',
			id: 'colour',
			label: t('tabs.menu.colour'),
			icon: createElement(PaletteIcon),
			items: [
				{
					type: 'checkbox',
					id: `${COLOUR_PREFIX}none`,
					label: t('tabs.colour.none'),
					icon: createElement(CircleIcon),
					checked: tab.colour === null,
				},
				...TAB_COLOURS.map((colour) => ({
					type: 'checkbox' as const,
					id: `${COLOUR_PREFIX}${colour}`,
					label: t(colourMessageId(colour)),
					checked: tab.colour === colour,
					icon: createElement(TabColourSwatch, { colour }),
				})),
			],
		},
		{
			type: 'submenu',
			id: 'addToGroup',
			label: t('tabs.menu.addToGroup'),
			icon: createElement(GroupIcon),
			items: [
				...groups
					.filter((group) => group.id !== tab.group)
					.map((group) => ({
						type: 'action' as const,
						id: `${GROUP_PREFIX}${group.id}`,
						label: group.name,
						icon: group.colour
							? createElement(TabColourSwatch, { colour: group.colour })
							: createElement(GroupIcon),
					})),
				...(groups.some((group) => group.id !== tab.group)
					? [{ type: 'separator' as const, id: 'sep-new-group' }]
					: []),
				{
					type: 'action' as const,
					id: 'newGroup',
					label: t('tabs.menu.newGroup'),
					icon: createElement(PlusIcon),
				},
			],
		},
		...(own
			? [
					{
						type: 'action' as const,
						id: 'removeFromGroup',
						label: t('tabs.menu.removeFromGroup'),
						icon: createElement(UngroupIcon),
					},
				]
			: []),
		{ type: 'separator', id: 'sep-copy' },
		{
			type: 'action',
			id: 'duplicate',
			label: t('tabs.menu.duplicate'),
			icon: createElement(CopyIcon),
		},
		{ type: 'separator', id: 'sep-window' },
		...windowMoveItems(others),
		{ type: 'separator', id: 'sep-close' },
		{
			type: 'action',
			id: 'close',
			label: t('tabs.menu.close'),
			shortcut: 'Ctrl+W',
			icon: createElement(CloseSmallIcon),
		},
		{
			type: 'action',
			id: 'closeOthers',
			label: t('tabs.menu.closeOthers'),
			icon: createElement(CloseOthersIcon),
		},
		{
			type: 'action',
			id: 'closeRight',
			label: t('tabs.menu.closeRight'),
			icon: createElement(CloseToRightIcon),
		},
		{ type: 'separator', id: 'sep-closed' },
		...closedTabItems(closed),
	];
}

/** The + button's menu; `connect` adds Connect to Server… where connections are available. */
export function plusMenuItems(closed: readonly ClosedTab[], connect = false): MenuItem[] {
	return [
		{
			type: 'action',
			id: 'newTab',
			label: t('tabs.plusMenu.newTab'),
			shortcut: 'Ctrl+T',
			icon: createElement(NewTabIcon),
		},
		{
			type: 'action',
			id: 'newTabHome',
			label: t('tabs.plusMenu.newTabHome'),
			icon: createElement(HomeIcon),
		},
		{
			type: 'action',
			id: 'newWindow',
			label: t('tabs.plusMenu.newWindow'),
			shortcut: 'Ctrl+Shift+N',
			icon: createElement(WindowIcon),
		},
		...(connect
			? [
					{
						type: 'action',
						id: 'connectToServer',
						label: t('cmd.connectToServer'),
						icon: createElement(ServerIcon),
					} satisfies MenuItem,
				]
			: []),
		{ type: 'separator', id: 'sep-closed' },
		...closedTabItems(closed),
	];
}

interface MenuContext {
	closed: readonly ClosedTab[];
	extras: TabExtras;
	/** The other windows the tab can move to, and the commands that move it. */
	windows?: {
		others: readonly WindowSummary[];
		actions: Pick<WindowActions, 'moveToNewWindow' | 'moveToWindow'>;
	};
	groups?: readonly Group[];
	groupActions?: GroupActions;
}

/** Runs what choosing `item` in the tab menu means for `tab`. */
export function runTabMenuItem(
	item: SelectableMenuItem,
	tab: TabSnapshot,
	actions: Pick<TabActions, 'close'>,
	{ closed, extras, groups = [], groupActions, windows }: MenuContext,
): void {
	if (windows && runWindowMoveItem(item, tab, windows.others, windows.actions)) return;
	if (item.id.startsWith(GROUP_PREFIX)) {
		const id = Number(item.id.slice(GROUP_PREFIX.length));
		const group = groups.find((candidate) => candidate.id === id);
		if (group) groupActions?.addTo(tab, group);
		return;
	}
	if (item.id.startsWith(COLOUR_PREFIX)) {
		const name = item.id.slice(COLOUR_PREFIX.length);
		const colour = TAB_COLOURS.find((candidate) => candidate === name) ?? null;
		extras.setColour(tab, colour);
		return;
	}
	if (runClosedItem(item, closed, extras)) return;
	switch (item.id) {
		case 'pin':
			return extras.pin(tab, true);
		case 'unpin':
			return extras.pin(tab, false);
		case 'newGroup':
			return groupActions?.newGroup(tab);
		case 'removeFromGroup':
			return groupActions?.removeFrom(tab);
		case 'duplicate':
			return extras.duplicate(tab);
		case 'close':
			return actions.close(tab.id);
		case 'closeOthers':
			return extras.closeOthers(tab);
		case 'closeRight':
			return extras.closeToRight(tab);
	}
}

/** Runs what choosing `item` in the + button menu means. */
export function runPlusMenuItem(
	item: SelectableMenuItem,
	actions: Pick<TabActions, 'newTab' | 'newTabAtHome'>,
	{
		closed,
		extras,
		windows,
	}: Pick<MenuContext, 'closed' | 'extras'> & {
		windows: Pick<WindowActions, 'newWindow'>;
	},
): void {
	if (runClosedItem(item, closed, extras)) return;
	if (item.id === 'newTab') actions.newTab();
	else if (item.id === 'newTabHome') actions.newTabAtHome();
	else if (item.id === 'newWindow') void windows.newWindow();
	else if (item.id === 'connectToServer') openConnectDialog();
}

function runClosedItem(
	item: SelectableMenuItem,
	closed: readonly ClosedTab[],
	extras: TabExtras,
): boolean {
	if (item.id === 'reopen') {
		extras.reopen();
		return true;
	}
	if (item.id.startsWith(CLOSED_PREFIX)) {
		const id = Number(item.id.slice(CLOSED_PREFIX.length));
		const entry = closed.find((candidate) => candidate.tab.id === id);
		if (entry) extras.reopen(entry);
		return true;
	}
	return false;
}
