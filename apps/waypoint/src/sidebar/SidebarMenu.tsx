// The context menu of a sidebar item: what Open, Open in New Tab, and each kind of item's own actions do
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { t, type MessageId } from '../i18n/messages';
import type { ItemMenuRequest } from './itemGestures';

export interface MenuActions {
	open(location: Location): void;
	openInNewTab(location: Location): void;
	startRename(location: Location): void;
	remove(location: Location): void;
	move(location: Location, by: number): void;
	add(location: Location): void;
}

interface SidebarMenuProps {
	request: ItemMenuRequest;
	/** Where the item sits among the favourites, and how many there are; `null` for an item that is not one. */
	favouritePosition: { index: number; count: number } | null;
	/** Whether the item's folder is already a favourite, which leaves nothing to add. */
	pinned: boolean;
	actions: MenuActions;
	onClose: () => void;
}

const LABELS: Record<ItemMenuRequest['kind'], MessageId> = {
	place: 'sidebar.menu.place',
	favourite: 'sidebar.menu.favourite',
	folder: 'sidebar.menu.folder',
};

export function SidebarMenu({
	request,
	favouritePosition,
	pinned,
	actions,
	onClose,
}: SidebarMenuProps) {
	const { kind, location } = request;
	const items: MenuItem[] = [
		{ type: 'action', id: 'open', label: t('menu.open') },
		{ type: 'action', id: 'openInNewTab', label: t('menu.openInNewTab') },
	];
	if (kind === 'favourite') {
		items.push(
			{ type: 'separator' },
			{ type: 'action', id: 'rename', label: t('menu.rename'), shortcut: 'F2' },
			{
				type: 'action',
				id: 'moveUp',
				label: t('menu.moveUp'),
				shortcut: 'Alt+↑',
				disabled: !favouritePosition || favouritePosition.index === 0,
			},
			{
				type: 'action',
				id: 'moveDown',
				label: t('menu.moveDown'),
				shortcut: 'Alt+↓',
				disabled: !favouritePosition || favouritePosition.index >= favouritePosition.count - 1,
			},
			{ type: 'separator' },
			{ type: 'action', id: 'remove', label: t('menu.removeFromFavourites') },
		);
	} else if (kind === 'folder') {
		items.push(
			{ type: 'separator' },
			{ type: 'action', id: 'add', label: t('menu.addToFavourites'), disabled: pinned },
		);
	}
	return (
		<ContextMenu
			items={items}
			position={request.position}
			ariaLabel={t(LABELS[kind])}
			openedWithKeyboard={request.keyboard}
			returnFocusTo={request.returnFocus}
			onClose={onClose}
			onSelect={(item) => {
				onClose();
				switch (item.id) {
					case 'open':
						return actions.open(location);
					case 'openInNewTab':
						return actions.openInNewTab(location);
					case 'rename':
						return actions.startRename(location);
					case 'moveUp':
						return actions.move(location, -1);
					case 'moveDown':
						return actions.move(location, 1);
					case 'remove':
						return actions.remove(location);
					case 'add':
						return actions.add(location);
				}
			}}
		/>
	);
}
