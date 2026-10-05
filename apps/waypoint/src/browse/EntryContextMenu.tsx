// The context menu of an entry: Open, Open in New Tab and Add to Favourites for folders, Copy Path, and the write commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem, SubmenuMenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { isArchiveEntry } from '../archives/archiveNames';
import { t } from '../i18n/messages';
import { PropertiesIcon } from '../inspector/InspectorIcons';
import { useOpenWithMenu } from '../openWith/useOpenWithMenu';
import { useShelfActions } from '../shelf/ShelfContext';
import { AddToShelfIcon } from '../shelf/ShelfIcons';
import type { ListingSession } from './useListingSession';
import { isFolder } from '../nav/useOpenEntry';
import { StarIcon } from '../icons/AppIcons';
import {
	CompressIcon,
	CopyIcon,
	CopyToIcon,
	CutIcon,
	DeleteForeverIcon,
	DuplicateIcon,
	EditIcon,
	ExtractIcon,
	FolderOpenIcon,
	LinkIcon,
	MoveToIcon,
	NewTabIcon,
	SplitPaneIcon,
	PasteIcon,
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
	/** Opens the folder in a new pane beside the one on show. */
	onOpenInSplit?: (entry: Entry, handle: ListingHandle) => void;
	onCopyPath: (entry: Entry, handle: ListingHandle) => void;
	onAddToFavourites: (entry: Entry, handle: ListingHandle) => void;
	/** The commands this menu may offer, by what the listing allows; omitted where nothing can be written. */
	commands?: Partial<Record<FileCommandId, CommandState>> | undefined;
	/** Runs one of the file commands on this entry's listing. */
	onCommand?: ((command: EntryCommand, entry: Entry) => void) | undefined;
	/** More than one entry is selected, so Rename Selected… (batch rename) is offered. */
	batchRename?: boolean | undefined;
	/** The listing the entry is in, whose selection Add to Shelf puts on the Shelf. */
	session?: ListingSession | null | undefined;
	/** Properties windows can be opened here, so "Properties in a Window" is listed. */
	propertiesWindow?: boolean | undefined;
}

/** The commands the entry menu can run. */
export type EntryCommand =
	| 'rename'
	| 'batchRename'
	| 'duplicate'
	| 'extractHere'
	| 'extractTo'
	| 'compress'
	| 'moveToTrash'
	| 'deletePermanently'
	| 'cut'
	| 'copy'
	| 'paste'
	| 'pasteInto'
	| 'copyTo'
	| 'moveTo'
	| 'copyToOtherPane'
	| 'moveToOtherPane'
	| 'properties'
	| 'propertiesWindow';

const ENTRY_COMMANDS: EntryCommand[] = [
	'rename',
	'batchRename',
	'duplicate',
	'extractHere',
	'extractTo',
	'compress',
	'moveToTrash',
	'deletePermanently',
	'cut',
	'copy',
	'paste',
	'pasteInto',
	'copyTo',
	'moveTo',
	'copyToOtherPane',
	'moveToOtherPane',
	'properties',
	'propertiesWindow',
];

/**
 * Cut, Copy and Paste, which lead the section Add to Favourites and Copy Path are in (Add to Shelf
 * joins it with the Shelf). Paste is Paste Into Folder on a folder, so a folder's menu says where
 * the files go; on a file it pastes into the folder the file is in, as Ctrl+V does.
 */
function clipboardItems(
	entry: Entry,
	commands: Partial<Record<FileCommandId, CommandState>>,
): MenuItem[] {
	const shown = (id: FileCommandId) => commands[id]?.visible === true;
	const disabled = (id: FileCommandId) => commands[id]?.enabled !== true;
	const folder = isFolder(entry);
	return [
		...(shown('cut')
			? [
					{
						type: 'action',
						id: 'cut',
						label: t('menu.cut'),
						shortcut: 'Ctrl+X',
						icon: <CutIcon />,
						disabled: disabled('cut'),
					} as const,
				]
			: []),
		...(shown('copy')
			? [
					{
						type: 'action',
						id: 'copy',
						label: t('menu.copy'),
						shortcut: 'Ctrl+C',
						icon: <CopyIcon />,
						disabled: disabled('copy'),
					} as const,
				]
			: []),
		...(shown('paste') && !folder
			? [
					{
						type: 'action',
						id: 'paste',
						label: t('menu.paste'),
						shortcut: 'Ctrl+V',
						icon: <PasteIcon />,
						disabled: disabled('paste'),
					} as const,
				]
			: []),
		...(shown('pasteInto') && folder
			? [
					{
						type: 'action',
						id: 'pasteInto',
						label: t('menu.pasteInto'),
						icon: <PasteIcon />,
						disabled: disabled('pasteInto'),
					} as const,
				]
			: []),
	];
}

/**
 * Copy To… and Move To… (the destination dialog), and in a pair Copy to Other Pane and Move to
 * Other Pane, which F5 and Shift+F5 run. Without a pair F5 asks as Copy To… does, so every key has
 * an item and every item has a key or a menu path.
 */
function transferItems(commands: Partial<Record<FileCommandId, CommandState>>): MenuItem[] {
	const shown = (id: FileCommandId) => commands[id]?.visible === true;
	const disabled = (id: FileCommandId) => commands[id]?.enabled !== true;
	return [
		...(shown('copyTo')
			? [
					{
						type: 'action',
						id: 'copyTo',
						label: t('menu.copyTo'),
						icon: <CopyToIcon />,
						disabled: disabled('copyTo'),
					} as const,
				]
			: []),
		...(shown('moveTo')
			? [
					{
						type: 'action',
						id: 'moveTo',
						label: t('menu.moveTo'),
						icon: <MoveToIcon />,
						disabled: disabled('moveTo'),
					} as const,
				]
			: []),
		...(shown('copyToOtherPane')
			? [
					{
						type: 'action',
						id: 'copyToOtherPane',
						label: t('menu.copyToOtherPane'),
						shortcut: 'F5',
						icon: <CopyToIcon />,
						disabled: disabled('copyToOtherPane'),
					} as const,
				]
			: []),
		...(shown('moveToOtherPane')
			? [
					{
						type: 'action',
						id: 'moveToOtherPane',
						label: t('menu.moveToOtherPane'),
						shortcut: 'Shift+F5',
						icon: <MoveToIcon />,
						disabled: disabled('moveToOtherPane'),
					} as const,
				]
			: []),
	];
}

/** Extract Here and Extract To…, offered on an archive (its entry is the one the keyboard is on). */
function extractItems(commands: Partial<Record<FileCommandId, CommandState>>): MenuItem[] {
	const shown = (id: FileCommandId) => commands[id]?.visible === true;
	const disabled = (id: FileCommandId) => commands[id]?.enabled !== true;
	return [
		...(shown('extractHere')
			? [
					{
						type: 'action',
						id: 'extractHere',
						label: t('menu.extractHere'),
						icon: <ExtractIcon />,
						disabled: disabled('extractHere'),
					} as const,
				]
			: []),
		...(shown('extractTo')
			? [
					{
						type: 'action',
						id: 'extractTo',
						label: t('menu.extractTo'),
						icon: <ExtractIcon />,
						disabled: disabled('extractTo'),
					} as const,
				]
			: []),
	];
}

/**
 * The write items, in the order of `docs/interactions.md`: Rename and Duplicate in their own
 * section, then the destructive ones last and in red. They are left out where the listing is
 * read-only. Compress and Tags join after Duplicate.
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
		...(shown('compress')
			? [
					{
						type: 'action',
						id: 'compress',
						label: t('menu.compress'),
						icon: <CompressIcon />,
						disabled: disabled('compress'),
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
	const transfer = transferItems(commands);
	const extract = extractItems(commands);
	return [
		...(editing.length > 0 ? [{ type: 'separator' } as const, ...editing] : []),
		...(extract.length > 0 ? [{ type: 'separator' } as const, ...extract] : []),
		...(transfer.length > 0 ? [{ type: 'separator' } as const, ...transfer] : []),
		...(destructive.length > 0 ? [{ type: 'separator' } as const, ...destructive] : []),
	];
}

/**
 * The entry menu's items; Open in New Tab and Add to Favourites are for folders only. `commands`
 * adds the write items the listing allows, and `batchRename` (more than one entry is selected)
 * adds Rename Selected… after Rename. `openWith` is the Open With ▸ submenu, which ends the first
 * section when the system can offer it.
 */
export function entryMenuItems(
	entry: Entry,
	commands?: Partial<Record<FileCommandId, CommandState>>,
	batchRename = false,
	openWith: SubmenuMenuItem | null = null,
	propertiesWindow = false,
): MenuItem[] {
	return [
		{
			type: 'action',
			id: 'open',
			label: t('menu.open'),
			shortcut: 'Enter',
			icon: <FolderOpenIcon />,
		},
		// An archive opens like a folder, so it is offered the same ways to open beside this one.
		...(isFolder(entry) || isArchiveEntry(entry)
			? [
					{
						type: 'action',
						id: 'openInNewTab',
						label: t('menu.openInNewTab'),
						icon: <NewTabIcon />,
					} as const,
					{
						type: 'action',
						id: 'openInSplit',
						label: t('menu.openInSplit'),
						icon: <SplitPaneIcon />,
					} as const,
					{
						type: 'action',
						id: 'openInNewWindow',
						label: t('menu.openInNewWindow'),
						icon: <WindowIcon />,
					} as const,
				]
			: []),
		...(openWith ? [openWith] : []),
		{ type: 'separator' },
		...(commands ? clipboardItems(entry, commands) : []),
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
		{ type: 'action', id: 'addToShelf', label: t('menu.addToShelf'), icon: <AddToShelfIcon /> },
		{ type: 'action', id: 'copyPath', label: t('menu.copyPath'), icon: <LinkIcon /> },
		...(commands ? writeItems(commands, batchRename) : []),
		{ type: 'separator' },
		{
			type: 'action',
			id: 'properties',
			label: t('menu.properties'),
			icon: <PropertiesIcon />,
		},
		...(propertiesWindow
			? [
					{
						type: 'action',
						id: 'propertiesWindow',
						label: t('menu.propertiesInWindow'),
						icon: <PropertiesIcon />,
						shortcut: 'Alt+Enter',
					} as const,
				]
			: []),
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
	onOpenInSplit,
	onCopyPath,
	onAddToFavourites,
	commands,
	onCommand,
	batchRename = false,
	session,
	propertiesWindow = false,
}: EntryContextMenuProps) {
	const shelf = useShelfActions();
	const openWith = useOpenWithMenu({ session, entry, handle });
	const items = entryMenuItems(entry, commands, batchRename, openWith.item, propertiesWindow);
	return (
		<ContextMenu
			items={items}
			position={position}
			ariaLabel={t('menu.entry.label')}
			openedWithKeyboard={keyboard}
			onClose={onClose}
			onSelect={(item) => {
				onClose();
				if (openWith.select(item.id)) return;
				if (item.id === 'open') onOpen(entry, handle);
				else if (item.id === 'openInNewTab') onOpenInNewTab(entry, handle);
				else if (item.id === 'openInSplit') onOpenInSplit?.(entry, handle);
				else if (item.id === 'openInNewWindow') onOpenInNewTab(entry, handle, true);
				else if (item.id === 'copyPath') onCopyPath(entry, handle);
				else if (item.id === 'addToFavourites') onAddToFavourites(entry, handle);
				else if (item.id === 'addToShelf') {
					if (session) void shelf?.addSelection(session);
				} else if (ENTRY_COMMANDS.includes(item.id as EntryCommand)) {
					onCommand?.(item.id as EntryCommand, entry);
				}
			}}
		/>
	);
}
