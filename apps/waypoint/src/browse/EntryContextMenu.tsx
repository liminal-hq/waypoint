// The context menu of an entry: Open, Open in New Tab and Add to Favourites for folders, Copy Path, and the write commands
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
import {
	DeleteForeverIcon,
	DuplicateIcon,
	EditIcon,
	FolderOpenIcon,
	LinkIcon,
	NewTabIcon,
	TrashIcon,
	WindowIcon,
} from '../icons/MenuIcons';
import type { CommandState, FileCommandId } from '../ops/fileCommands';

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
	/** The commands this menu may offer, by what the listing allows; omitted where nothing can be written. */
	commands?: Partial<Record<FileCommandId, CommandState>> | undefined;
	/** Runs one of the file commands on this entry's listing. */
	onCommand?: ((command: EntryCommand, entry: Entry) => void) | undefined;
	/** More than one entry is selected, so Rename Selected… (batch rename) is offered. */
	batchRename?: boolean | undefined;
}

/** The commands the entry menu can run. */
export type EntryCommand =
	'rename' | 'batchRename' | 'duplicate' | 'moveToTrash' | 'deletePermanently';

const ENTRY_COMMANDS: EntryCommand[] = [
	'rename',
	'batchRename',
	'duplicate',
	'moveToTrash',
	'deletePermanently',
];

/**
 * The write items, in the order of `docs/interactions.md`: Rename and Duplicate in their own
 * section, then the destructive ones last and in red. They are left out where the listing is
 * read-only. Cut, Copy, Paste and Add to Shelf join the section before Copy Path (milestone 4,
 * slices 09 and 13), Compress and Tags join after Duplicate.
 */
function writeItems(
	commands: Partial<Record<FileCommandId, CommandState>>,
	batchRename: boolean,
): MenuItem[] {
	const shown = (id: FileCommandId) => commands[id]?.visible === true;
	const disabled = (id: FileCommandId) => commands[id]?.enabled !== true;
	const editing: MenuItem[] = [
		...(shown('rename')
			? [
					{
						type: 'action',
						id: 'rename',
						label: t('menu.rename'),
						shortcut: 'F2',
						icon: <EditIcon />,
						disabled: disabled('rename'),
					} as const,
				]
			: []),
		...(shown('rename') && batchRename
			? [
					{
						type: 'action',
						id: 'batchRename',
						label: t('menu.renameSelected'),
						shortcut: 'Ctrl+F2',
						icon: <EditIcon />,
					} as const,
				]
			: []),
		...(shown('duplicate')
			? [
					{
						type: 'action',
						id: 'duplicate',
						label: t('menu.duplicate'),
						shortcut: 'Ctrl+Shift+D',
						icon: <DuplicateIcon />,
						disabled: disabled('duplicate'),
					} as const,
				]
			: []),
	];
	const destructive: MenuItem[] = [
		...(shown('moveToTrash')
			? [
					{
						type: 'action',
						id: 'moveToTrash',
						label: t('menu.moveToTrash'),
						shortcut: 'Delete',
						icon: <TrashIcon />,
						danger: true,
						disabled: disabled('moveToTrash'),
					} as const,
				]
			: []),
		...(shown('deletePermanently')
			? [
					{
						type: 'action',
						id: 'deletePermanently',
						label: t('menu.deletePermanently'),
						shortcut: 'Shift+Delete',
						icon: <DeleteForeverIcon />,
						danger: true,
						disabled: disabled('deletePermanently'),
					} as const,
				]
			: []),
	];
	return [
		...(editing.length > 0 ? [{ type: 'separator' } as const, ...editing] : []),
		...(destructive.length > 0 ? [{ type: 'separator' } as const, ...destructive] : []),
	];
}

/**
 * The entry menu's items; Open in New Tab and Add to Favourites are for folders only. `commands`
 * adds the write items the listing allows, and `batchRename` (more than one entry is selected)
 * adds Rename Selected… after Rename.
 */
export function entryMenuItems(
	entry: Entry,
	commands?: Partial<Record<FileCommandId, CommandState>>,
	batchRename = false,
): MenuItem[] {
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
		...(commands ? writeItems(commands, batchRename) : []),
	];
}

/**
 * The entry's actions. Open in New Tab and Add to Favourites appear for folders only, and the
 * menu acts on the entry that was right-clicked, which the list has just selected (Rename on that
 * entry, the other write commands on the whole selection).
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
	commands,
	onCommand,
	batchRename = false,
}: EntryContextMenuProps) {
	const items = entryMenuItems(entry, commands, batchRename);
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
				else if (ENTRY_COMMANDS.includes(item.id as EntryCommand)) {
					onCommand?.(item.id as EntryCommand, entry);
				}
			}}
		/>
	);
}
