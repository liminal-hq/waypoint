// A group's right-click menu: the twelve items of SPEC section 5.2, over the shared context menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type {
	MenuItem,
	MenuPosition,
	SelectableMenuItem,
} from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Group } from '@liminal-hq/waypoint-protocol/generated/Group';
import type { GroupSort } from '@liminal-hq/waypoint-protocol/generated/GroupSort';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { createElement } from 'react';
import { t } from '../i18n/messages';
import { useGroupActions, type GroupActions } from './groupActions';
import { colourMessageId, TAB_COLOURS } from './tabColours';
import { TabColourSwatch } from './TabColourSwatch';
import { useOtherWindows } from './useOtherWindows';
import { useWindowActions } from './windowActions';
import { runWindowMoveChoice, windowMoveItems } from './windowMoveMenu';

const COLOUR_PREFIX = 'colour:';
const SORT_PREFIX = 'sort:';

const SORTS: {
	by: GroupSort;
	message: 'groups.menu.sortName' | 'groups.menu.sortLocation' | 'groups.menu.sortLocal';
}[] = [
	{ by: 'name', message: 'groups.menu.sortName' },
	{ by: 'location', message: 'groups.menu.sortLocation' },
	{ by: 'localFirst', message: 'groups.menu.sortLocal' },
];

/** What the menu needs to know beyond the group itself. */
export interface GroupMenuState {
	/** The group's tabs are pinned. */
	pinned: boolean;
	/** Another group exists in this window, so Collapse All Other Groups has something to do. */
	hasOthers: boolean;
}

/** The group menu's items, in SPEC order. */
export function groupMenuItems(
	group: Group,
	{ pinned, hasOthers }: GroupMenuState,
	others: readonly WindowSummary[] = [],
): MenuItem[] {
	return [
		{ type: 'action', id: 'rename', label: t('groups.menu.rename'), shortcut: 'F2' },
		{
			type: 'submenu',
			id: 'colour',
			label: t('groups.menu.colour'),
			items: [
				{
					type: 'checkbox',
					id: `${COLOUR_PREFIX}none`,
					label: t('tabs.colour.none'),
					checked: group.colour === null,
				},
				...TAB_COLOURS.map((colour) => ({
					type: 'checkbox' as const,
					id: `${COLOUR_PREFIX}${colour}`,
					label: t(colourMessageId(colour)),
					checked: group.colour === colour,
					icon: createElement(TabColourSwatch, { colour }),
				})),
			],
		},
		{
			type: 'action',
			id: group.collapsed ? 'expand' : 'collapse',
			label: t(group.collapsed ? 'groups.menu.expand' : 'groups.menu.collapse'),
		},
		{
			type: 'action',
			id: 'collapseOthers',
			label: t('groups.menu.collapseOthers'),
			disabled: !hasOthers,
		},
		{ type: 'separator', id: 'sep-tabs' },
		{ type: 'action', id: 'newTab', label: t('groups.menu.newTab') },
		{
			type: 'action',
			id: pinned ? 'unpin' : 'pin',
			label: t(pinned ? 'groups.menu.unpin' : 'groups.menu.pin'),
		},
		{
			type: 'submenu',
			id: 'sort',
			label: t('groups.menu.sort'),
			items: SORTS.map(({ by, message }) => ({
				type: 'action' as const,
				id: `${SORT_PREFIX}${by}`,
				label: t(message),
			})),
		},
		{ type: 'separator', id: 'sep-copy' },
		{ type: 'action', id: 'duplicate', label: t('groups.menu.duplicate') },
		{
			type: 'action',
			id: 'saveWorkspace',
			label: t('groups.menu.saveWorkspace'),
		},
		...windowMoveItems(others, {
			newWindow: t('groups.menu.moveWindow'),
			toWindow: t('groups.menu.moveToWindow'),
		}),
		{ type: 'separator', id: 'sep-close' },
		{ type: 'action', id: 'ungroup', label: t('groups.menu.ungroup') },
		{ type: 'action', id: 'close', label: t('groups.menu.close'), danger: true },
	];
}

/** Runs what choosing `item` means for `group`. */
export function runGroupMenuItem(
	item: SelectableMenuItem,
	group: Group,
	actions: GroupActions,
	startRename: () => void,
	others: readonly WindowSummary[] = [],
): void {
	const moved = runWindowMoveChoice(item, others, {
		toNewWindow: () => actions.moveToNewWindow(group),
		toWindow: (target) => actions.moveToWindow(group, target),
	});
	if (moved) return;
	if (item.id.startsWith(COLOUR_PREFIX)) {
		const name = item.id.slice(COLOUR_PREFIX.length);
		actions.setColour(group, TAB_COLOURS.find((candidate) => candidate === name) ?? null);
		return;
	}
	if (item.id.startsWith(SORT_PREFIX)) {
		const by = SORTS.find((candidate) => `${SORT_PREFIX}${candidate.by}` === item.id)?.by;
		if (by) actions.sort(group, by);
		return;
	}
	switch (item.id) {
		case 'rename':
			return startRename();
		case 'collapse':
			return actions.setCollapsed(group, true);
		case 'expand':
			return actions.setCollapsed(group, false);
		case 'collapseOthers':
			return actions.collapseOthers(group);
		case 'newTab':
			return actions.newTabIn(group);
		case 'pin':
			return actions.setPinned(group, true);
		case 'unpin':
			return actions.setPinned(group, false);
		case 'duplicate':
			return actions.duplicate(group);
		case 'saveWorkspace':
			return actions.saveAsWorkspace(group);
		case 'ungroup':
			return actions.ungroup(group);
		case 'close':
			return actions.close(group);
	}
}

interface GroupMenuProps extends GroupMenuState {
	group: Group;
	position: MenuPosition;
	openedWithKeyboard: boolean;
	/** What takes the focus back when the menu closes. */
	returnFocusTo: HTMLElement | null;
	onClose: () => void;
	onRename: () => void;
}

/** Opened by right-click, or by the Menu key or Shift+F10 on a focused chip. */
export function GroupMenu({ group, pinned, hasOthers, onRename, ...rest }: GroupMenuProps) {
	const actions = useGroupActions();
	const windows = useWindowActions();
	const others = useOtherWindows(windows);
	return (
		<ContextMenu
			{...rest}
			ariaLabel={t('groups.menu.label')}
			items={groupMenuItems(group, { pinned, hasOthers }, others)}
			onSelect={(item) => runGroupMenuItem(item, group, actions, onRename, others)}
		/>
	);
}
