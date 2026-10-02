// The context menu of an item in the Trash: Restore and Delete Permanently, never Open
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { t } from '../i18n/messages';
import { RestoreIcon, TrashIcon } from '../icons/MenuIcons';

/** The Trash item menu's items. An item cannot be opened, renamed or copied from here. */
export function trashEntryMenuItems(): MenuItem[] {
	return [
		{ type: 'action', id: 'restore', label: t('menu.restore'), icon: <RestoreIcon /> },
		{ type: 'separator' },
		{
			type: 'action',
			id: 'delete',
			label: t('menu.deletePermanently'),
			shortcut: 'Delete',
			danger: true,
			icon: <TrashIcon />,
		},
	];
}

interface TrashEntryMenuProps {
	position: { x: number; y: number };
	keyboard: boolean;
	onRestore: () => void;
	onDelete: () => void;
	onClose: () => void;
}

/** Acts on the selection, which the list has just made of the item that was right-clicked. */
export function TrashEntryMenu({
	position,
	keyboard,
	onRestore,
	onDelete,
	onClose,
}: TrashEntryMenuProps) {
	return (
		<ContextMenu
			items={trashEntryMenuItems()}
			position={position}
			ariaLabel={t('menu.entry.label')}
			openedWithKeyboard={keyboard}
			onClose={onClose}
			onSelect={(item) => {
				onClose();
				if (item.id === 'restore') onRestore();
				else if (item.id === 'delete') onDelete();
			}}
		/>
	);
}
