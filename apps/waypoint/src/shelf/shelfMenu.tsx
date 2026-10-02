// The Shelf's menus: what one or more items offer, and the panel's own (Clear Shelf)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { t } from '../i18n/messages';
import { CopyIcon, FolderOpenIcon, LinkIcon, TrashIcon } from '../icons/MenuIcons';

/** What a Shelf item's menu can do. */
export type ShelfItemCommand = 'open' | 'reveal' | 'copyPath' | 'copyFiles' | 'remove';

export const SHELF_ITEM_COMMANDS: readonly ShelfItemCommand[] = [
	'open',
	'reveal',
	'copyPath',
	'copyFiles',
	'remove',
];

/**
 * The item menu: Open, Reveal in Folder, Copy Path and Remove from Shelf, in the order of
 * `docs/interactions.md`, plus Copy (the file onto the shared clipboard, so Paste puts it in a
 * folder). Open and Reveal act on one item, so they are left out when several are selected.
 * Remove takes the items off the Shelf and nothing else; its label says so.
 */
export function shelfItemMenuItems(selected: number, canCopyFiles: boolean): MenuItem[] {
	const one = selected === 1;
	return [
		...(one
			? ([
					{
						type: 'action',
						id: 'open',
						label: t('shelf.menu.open'),
						shortcut: 'Enter',
						icon: <FolderOpenIcon />,
					},
					{
						type: 'action',
						id: 'reveal',
						label: t('shelf.menu.reveal'),
						icon: <FolderOpenIcon />,
					},
				] satisfies MenuItem[])
			: []),
		...(canCopyFiles
			? ([
					{
						type: 'action',
						id: 'copyFiles',
						label: t('menu.copy'),
						shortcut: 'Ctrl+C',
						icon: <CopyIcon />,
					},
				] satisfies MenuItem[])
			: []),
		{ type: 'action', id: 'copyPath', label: t('shelf.menu.copyPath'), icon: <LinkIcon /> },
		{ type: 'separator', id: 'sep-remove' },
		{
			type: 'action',
			id: 'remove',
			label: t('shelf.menu.remove'),
			shortcut: 'Delete',
			title: t('shelf.remove.title'),
			icon: <TrashIcon />,
			danger: true,
		},
	];
}

/** The panel's options menu. */
export function shelfPanelMenuItems(count: number): MenuItem[] {
	return [
		{
			type: 'action',
			id: 'clear',
			label: t('shelf.clear'),
			title: t('shelf.remove.title'),
			icon: <TrashIcon />,
			danger: true,
			disabled: count === 0,
		},
	];
}
