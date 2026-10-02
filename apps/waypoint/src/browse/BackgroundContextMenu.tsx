// The context menu of the file area's empty space: New, Undo and Redo, sort and hidden files
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { ReactNode } from 'react';
import type { SortKey } from '@liminal-hq/waypoint-protocol/generated/SortKey';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import { FolderTabIcon } from '../icons/AppIcons';
import {
	ArrowDownIcon,
	ClockIcon,
	EyeIcon,
	NewFileIcon,
	NewFolderIcon,
	PasteIcon,
	RedoIcon,
	SizeIcon,
	TagIcon,
	TextIcon,
	TrashIcon,
	UndoIcon,
} from '../icons/MenuIcons';
import { t, tf, type MessageId } from '../i18n/messages';
import type { CommandState, FileCommandId } from '../ops/fileCommands';
import type { ListingSession } from './useListingSession';

const SORT_KEYS: Array<{ key: SortKey; label: MessageId; icon: ReactNode }> = [
	{ key: 'name', label: 'menu.sort.name', icon: <TextIcon /> },
	{ key: 'size', label: 'menu.sort.size', icon: <SizeIcon /> },
	{ key: 'modified', label: 'menu.sort.modified', icon: <ClockIcon /> },
	{ key: 'kind', label: 'menu.sort.kind', icon: <TagIcon /> },
	{ key: 'deleted', label: 'menu.sort.deleted', icon: <ClockIcon /> },
];

/** The columns the Trash sorts by: its items have no modified time or kind of their own to show. */
const TRASH_SORT_KEYS: readonly SortKey[] = ['name', 'size', 'deleted'];

/** What the Trash's empty-space menu needs: how many items it holds, for the Empty Trash item. */
export interface TrashBackground {
	count: number;
}

/** What the file commands add to the empty-space menu. */
export interface BackgroundCommands {
	states: Partial<Record<FileCommandId, CommandState>>;
	/** The history's newest entries' words ("Move 3 items to Trash"), for "Undo Move 3 items to Trash". */
	undoLabel: string | null;
	redoLabel: string | null;
}

/** The commands the empty-space menu can run. */
export type BackgroundCommand = 'newFolder' | 'newFile' | 'paste' | 'undo' | 'redo';

const BACKGROUND_COMMANDS: BackgroundCommand[] = ['newFolder', 'newFile', 'paste', 'undo', 'redo'];

/**
 * New, Paste and the history, ahead of the view items. Paste follows New and, like it, is left out
 * where the listing is read-only (disabled while the clipboard is empty); Undo and Redo are always listed and disabled when there is
 * nothing to do.
 */
function commandItems({ states, undoLabel, redoLabel }: BackgroundCommands): MenuItem[] {
	const newItems: MenuItem[] = [
		...(states.newFolder?.visible
			? [
					{
						type: 'action',
						id: 'newFolder',
						label: t('menu.new.folder'),
						shortcut: 'F7',
						icon: <NewFolderIcon />,
					} as const,
				]
			: []),
		...(states.newFile?.visible
			? [
					{
						type: 'action',
						id: 'newFile',
						label: t('menu.new.file'),
						shortcut: 'Shift+F7',
						icon: <NewFileIcon />,
					} as const,
				]
			: []),
	];
	const history: MenuItem[] = [
		...(states.undo?.visible
			? [
					{
						type: 'action',
						id: 'undo',
						label: undoLabel ? tf('menu.undoNamed', { label: undoLabel }) : t('menu.undo'),
						shortcut: 'Ctrl+Z',
						icon: <UndoIcon />,
						disabled: !states.undo.enabled,
					} as const,
					{
						type: 'action',
						id: 'redo',
						label: redoLabel ? tf('menu.redoNamed', { label: redoLabel }) : t('menu.redo'),
						shortcut: 'Ctrl+Shift+Z',
						icon: <RedoIcon />,
						disabled: !states.redo?.enabled,
					} as const,
				]
			: []),
	];
	const paste: MenuItem[] = states.paste?.visible
		? [
				{
					type: 'action',
					id: 'paste',
					label: t('menu.paste'),
					shortcut: 'Ctrl+V',
					icon: <PasteIcon />,
					disabled: !states.paste.enabled,
				} as const,
			]
		: [];
	return [
		...(newItems.length > 0 || paste.length > 0
			? [
					...(newItems.length > 0
						? [
								{
									type: 'submenu',
									id: 'new',
									label: t('menu.new'),
									icon: <NewFolderIcon />,
									items: newItems,
								} as const,
							]
						: []),
					...paste,
					{ type: 'separator' } as const,
				]
			: []),
		...(history.length > 0 ? [...history, { type: 'separator' } as const] : []),
	];
}

/** What the Trash's empty-space menu and the file commands add to the plain one; each is absent where it does not apply. */
export interface BackgroundExtras {
	trash?: TrashBackground | null;
	commands?: BackgroundCommands | undefined;
}

/**
 * The empty-space menu's items: the file commands, sort (when a listing is open) and the
 * hidden-files toggle. In the Trash (read only, so no commands) the sort keys are Name, Size and
 * Date deleted, there is no hidden-files toggle (nothing is hidden), and Empty Trash comes last.
 */
export function backgroundMenuItems(
	sort: SortSpec | undefined,
	showHidden: boolean,
	{ trash = null, commands }: BackgroundExtras = {},
): MenuItem[] {
	const keys = SORT_KEYS.filter(({ key }) =>
		trash ? TRASH_SORT_KEYS.includes(key) : key !== 'deleted',
	);
	const items: MenuItem[] = [
		...(commands && !trash ? commandItems(commands) : []),
		...(sort
			? ([
					{ type: 'section', label: t('menu.sortBy') },
					...keys.map(({ key, label, icon }): MenuItem => ({
						type: 'checkbox',
						id: `sort:${key}`,
						label: t(label),
						icon,
						checked: sort.key === key,
					})),
					{ type: 'separator' },
					{
						type: 'checkbox',
						id: 'descending',
						label: t('menu.sort.descending'),
						icon: <ArrowDownIcon />,
						checked: sort.descending,
					},
					{
						type: 'checkbox',
						id: 'foldersFirst',
						label: t('menu.sort.foldersFirst'),
						icon: <FolderTabIcon />,
						checked: sort.directoriesFirst,
					},
					{ type: 'separator' },
				] as MenuItem[])
			: []),
	];
	if (!trash) {
		items.push({
			type: 'checkbox',
			id: 'showHidden',
			label: t('menu.showHidden'),
			icon: <EyeIcon />,
			checked: showHidden,
			shortcut: 'Ctrl+H',
		});
		return items;
	}
	// The sort block ends in a separator that the Empty Trash item follows.
	items.push({
		type: 'action',
		id: 'emptyTrash',
		label: t('menu.emptyTrash'),
		icon: <TrashIcon />,
		danger: true,
		disabled: trash.count === 0,
	});
	return items;
}

interface BackgroundContextMenuProps {
	session: ListingSession | null;
	showHidden: boolean;
	position: { x: number; y: number };
	keyboard: boolean;
	onToggleHidden: () => void;
	/** In the Trash: Empty Trash asks, then empties it. Absent elsewhere. */
	onEmptyTrash?: (() => void) | undefined;
	onClose: () => void;
	commands?: BackgroundCommands | undefined;
	onCommand?: ((command: BackgroundCommand) => void) | undefined;
}

/**
 * Sort by, direction, folders first and hidden files, for the folder as a whole. Sorting acts on
 * the open listing (Rust re-sorts it); hidden files are a window preference the host applies to
 * every listing.
 */
export function BackgroundContextMenu({
	session,
	showHidden,
	position,
	keyboard,
	onToggleHidden,
	onEmptyTrash,
	onClose,
	commands,
	onCommand,
}: BackgroundContextMenuProps) {
	const inTrash = session?.model.layout === 'trash';
	const items = backgroundMenuItems(session?.model.sort, showHidden, {
		trash: inTrash ? { count: session.model.count } : null,
		commands: inTrash ? undefined : commands,
	});

	return (
		<ContextMenu
			items={items}
			position={position}
			ariaLabel={t('menu.background.label')}
			openedWithKeyboard={keyboard}
			onClose={onClose}
			onSelect={(item) => {
				onClose();
				const model = session?.model;
				if (item.id === 'showHidden') return onToggleHidden();
				if (item.id === 'emptyTrash') return onEmptyTrash?.();
				if (BACKGROUND_COMMANDS.includes(item.id as BackgroundCommand)) {
					return onCommand?.(item.id as BackgroundCommand);
				}
				if (!model) return;
				const current = model.sort;
				if (item.id.startsWith('sort:')) {
					const key = item.id.slice('sort:'.length) as SortKey;
					// The active key stays as it is (Descending is its own item); a new key starts ascending.
					if (key !== current.key) void model.setSort({ ...current, key, descending: false });
				} else if (item.id === 'descending') {
					void model.setSort({ ...current, descending: !current.descending });
				} else if (item.id === 'foldersFirst') {
					void model.setSort({ ...current, directoriesFirst: !current.directoriesFirst });
				}
			}}
		/>
	);
}
