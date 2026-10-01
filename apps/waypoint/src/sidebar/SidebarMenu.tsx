// The context menu of a sidebar item: what Open, Open in New Tab, and each kind of item's own actions do
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { StarIcon, UpIcon } from '../icons/AppIcons';
import {
	ArrowDownIcon,
	EditIcon,
	FolderOpenIcon,
	NewTabIcon,
	StarOffIcon,
	WindowIcon,
} from '../icons/MenuIcons';
import { t, type MessageId } from '../i18n/messages';
import type { ItemMenuRequest } from './itemGestures';

export interface MenuActions {
	open(location: Location): void;
	openInNewTab(location: Location, inNewWindow?: boolean): void;
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
	/** Whether a favourite can be renamed (not in a workspace). */
	canRename: boolean;
	actions: MenuActions;
	onClose: () => void;
}

const LABELS: Record<ItemMenuRequest['kind'], MessageId> = {
	place: 'sidebar.menu.place',
	favourite: 'sidebar.menu.favourite',
	folder: 'sidebar.menu.folder',
};

/** The sidebar item menu's items: the common three, then what each kind of item adds. */
export function sidebarMenuItems(
	kind: ItemMenuRequest['kind'],
	{
		favouritePosition,
		pinned,
		canRename,
	}: Pick<SidebarMenuProps, 'favouritePosition' | 'pinned' | 'canRename'>,
): MenuItem[] {
	const items: MenuItem[] = [
		{ type: 'action', id: 'open', label: t('menu.open'), icon: <FolderOpenIcon /> },
		{ type: 'action', id: 'openInNewTab', label: t('menu.openInNewTab'), icon: <NewTabIcon /> },
		{
			type: 'action',
			id: 'openInNewWindow',
			label: t('menu.openInNewWindow'),
			icon: <WindowIcon />,
		},
	];
	if (kind === 'favourite') {
		items.push(
			{ type: 'separator' },
			...(canRename
				? [
						{
							type: 'action' as const,
							id: 'rename',
							label: t('menu.rename'),
							shortcut: 'F2',
							icon: <EditIcon />,
						},
					]
				: []),
			{
				type: 'action',
				id: 'moveUp',
				label: t('menu.moveUp'),
				icon: <UpIcon />,
				shortcut: 'Alt+↑',
				disabled: !favouritePosition || favouritePosition.index === 0,
			},
			{
				type: 'action',
				id: 'moveDown',
				label: t('menu.moveDown'),
				icon: <ArrowDownIcon />,
				shortcut: 'Alt+↓',
				disabled: !favouritePosition || favouritePosition.index >= favouritePosition.count - 1,
			},
			{ type: 'separator' },
			{
				type: 'action',
				id: 'remove',
				label: t('menu.removeFromFavourites'),
				danger: true,
				icon: <StarOffIcon />,
			},
		);
	} else if (kind === 'folder') {
		items.push(
			{ type: 'separator' },
			{
				type: 'action',
				id: 'add',
				label: t('menu.addToFavourites'),
				disabled: pinned,
				icon: <StarIcon />,
			},
		);
	}
	return items;
}

export function SidebarMenu({
	request,
	favouritePosition,
	pinned,
	canRename,
	actions,
	onClose,
}: SidebarMenuProps) {
	const { kind, location } = request;
	const items = sidebarMenuItems(kind, { favouritePosition, pinned, canRename });
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
					case 'openInNewWindow':
						return actions.openInNewTab(location, true);
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
