// The context menu of an entry: Open, Open in New Tab and Add to Favourites for folders, Copy Path
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { t } from '../i18n/messages';
import { isFolder } from '../nav/useOpenEntry';
import { StarIcon } from '../icons/AppIcons';
import { FolderOpenIcon, LinkIcon, NewTabIcon, WindowIcon } from '../icons/MenuIcons';

interface EntryContextMenuProps {
	entry: Entry;
	handle: ListingHandle;
	position: { x: number; y: number };
	keyboard: boolean;
	onClose: () => void;
	onOpen: (entry: Entry, handle: ListingHandle) => void;
	onOpenInNewTab: (entry: Entry, handle: ListingHandle, inNewWindow?: boolean) => void;
	onCopyPath: (entry: Entry, handle: ListingHandle) => void;
	onAddToFavourites: (entry: Entry, handle: ListingHandle) => void;
}

/** The entry menu's items; Open in New Tab and Add to Favourites are for folders only. */
export function entryMenuItems(entry: Entry): MenuItem[] {
	return [
		{
			type: 'action',
			id: 'open',
			label: t('menu.open'),
			shortcut: 'Enter',
			icon: <FolderOpenIcon />,
		},
		...(isFolder(entry)
			? [
					{
						type: 'action',
						id: 'openInNewTab',
						label: t('menu.openInNewTab'),
						icon: <NewTabIcon />,
					} as const,
					{
						type: 'action',
						id: 'openInNewWindow',
						label: t('menu.openInNewWindow'),
						icon: <WindowIcon />,
					} as const,
				]
			: []),
		{ type: 'separator' },
		...(isFolder(entry)
			? [
					{
						type: 'action',
						id: 'addToFavourites',
						label: t('menu.addToFavourites'),
						icon: <StarIcon />,
					} as const,
				]
			: []),
		{ type: 'action', id: 'copyPath', label: t('menu.copyPath'), icon: <LinkIcon /> },
	];
}

/**
 * The entry's actions while nothing writes to disk yet. Open in New Tab and Add to Favourites appear for folders only,
 * and the menu acts on the entry that was right-clicked, which the list has just selected.
 */
export function EntryContextMenu({
	entry,
	handle,
	position,
	keyboard,
	onClose,
	onOpen,
	onOpenInNewTab,
	onCopyPath,
	onAddToFavourites,
}: EntryContextMenuProps) {
	const items = entryMenuItems(entry);
	return (
		<ContextMenu
			items={items}
			position={position}
			ariaLabel={t('menu.entry.label')}
			openedWithKeyboard={keyboard}
			onClose={onClose}
			onSelect={(item) => {
				onClose();
				if (item.id === 'open') onOpen(entry, handle);
				else if (item.id === 'openInNewTab') onOpenInNewTab(entry, handle);
				else if (item.id === 'openInNewWindow') onOpenInNewTab(entry, handle, true);
				else if (item.id === 'copyPath') onCopyPath(entry, handle);
				else if (item.id === 'addToFavourites') onAddToFavourites(entry, handle);
			}}
		/>
	);
}
