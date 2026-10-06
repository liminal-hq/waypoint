// The context menu of the file area's empty space: New, Undo and Redo, sort, grouping and hidden files
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { ReactNode } from 'react';
import type { SortKey } from '@liminal-hq/waypoint-protocol/generated/SortKey';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import { FolderTabIcon } from '../icons/AppIcons';
import { useRepository } from '../git/GitContext';
import { PropertiesIcon } from '../inspector/InspectorIcons';
import {
	ArrowDownIcon,
	ClockIcon,
	EyeIcon,
	GitIcon,
	NewFileIcon,
	NewFolderIcon,
	PasteIcon,
	RedoIcon,
	RestoreIcon,
	SizeIcon,
	SortIcon,
	TagIcon,
	TextIcon,
	TrashIcon,
	UndoIcon,
} from '../icons/MenuIcons';
import { t, tf, type MessageId } from '../i18n/messages';
import type { CommandState, FileCommandId } from '../ops/fileCommands';
import { groupBySubmenu, groupFromMenuId } from './groupMenu';
import type { ListingSession } from './useListingSession';

const SORT_KEYS: Array<{ key: SortKey; label: MessageId; icon: ReactNode }> = [
	{ key: 'name', label: 'menu.sort.name', icon: <TextIcon /> },
	{ key: 'size', label: 'menu.sort.size', icon: <SizeIcon /> },
	{ key: 'modified', label: 'menu.sort.modified', icon: <ClockIcon /> },
	{ key: 'kind', label: 'menu.sort.kind', icon: <TagIcon /> },
	{ key: 'deleted', label: 'menu.sort.deleted', icon: <ClockIcon /> },
	{ key: 'git', label: 'menu.sort.git', icon: <GitIcon /> },
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
export type BackgroundCommand = 'newFolder' | 'newFile' | 'paste' | 'undo' | 'redo' | 'properties';

const BACKGROUND_COMMANDS: BackgroundCommand[] = [
	'newFolder',
	'newFile',
	'paste',
	'undo',
	'redo',
	'properties',
];

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
	/**
	 * What the folder remembers about its view: `remembered` offers Reset This Folder's View,
	 * `default` lists it disabled, and absent (remembering is off, or this is not a folder that
	 * can remember) leaves it out.
	 */
	folderView?: 'default' | 'remembered' | undefined;
	/** The folder is in a Git working tree, so Git status is one of the sort keys. */
	git?: boolean | undefined;
}

/**
 * The empty-space menu's items: the file commands, the Sort by and Group by submenus (when a
 * listing is open) and the hidden-files toggle. In the Trash (read only, so no commands) the sort
 * keys are Name, Size and Date deleted, there is no Group by and no hidden-files toggle (nothing
 * is hidden), and Empty Trash comes last.
 */
export function backgroundMenuItems(
	sort: SortSpec | undefined,
	showHidden: boolean,
	{ trash = null, commands, folderView, git = false }: BackgroundExtras = {},
): MenuItem[] {
	const keys = SORT_KEYS.filter(({ key }) =>
		trash ? TRASH_SORT_KEYS.includes(key) : key !== 'deleted' && (key !== 'git' || git),
	);
	const items: MenuItem[] = [
		...(commands && !trash ? commandItems(commands) : []),
		...(sort
			? ([
					{
						type: 'submenu',
						id: 'sortBy',
						label: t('menu.sortBy'),
						icon: <SortIcon />,
						items: [
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
						],
					},
					// Grouping has no meaning in the Trash, whose items keep their own columns.
					...(trash ? [] : [groupBySubmenu(sort)]),
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
		if (folderView) {
			items.push({
				type: 'action',
				id: 'resetFolderView',
				label: t('cmd.resetFolderView'),
				icon: <RestoreIcon />,
				disabled: folderView === 'default',
			});
		}
		items.push(
			{ type: 'separator' },
			{ type: 'action', id: 'properties', label: t('menu.properties'), icon: <PropertiesIcon /> },
		);
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
	/** What the folder remembers about its view, and the reset the menu offers for it. */
	folderView?: 'default' | 'remembered' | undefined;
	onResetFolderView?: (() => void) | undefined;
	onCommand?: ((command: BackgroundCommand) => void) | undefined;
}

/**
 * Sort by, direction, folders first, group by and hidden files, for the folder as a whole. Sorting acts on
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
	folderView,
	onResetFolderView,
	onCommand,
}: BackgroundContextMenuProps) {
	const inTrash = session?.model.layout === 'trash';
	const repository = useRepository(inTrash ? undefined : session?.model.location);
	const items = backgroundMenuItems(session?.model.sort, showHidden, {
		trash: inTrash ? { count: session.model.count } : null,
		commands: inTrash ? undefined : commands,
		folderView: inTrash ? undefined : folderView,
		git: repository !== null,
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
				if (item.id === 'resetFolderView') return onResetFolderView?.();
				if (BACKGROUND_COMMANDS.includes(item.id as BackgroundCommand)) {
					return onCommand?.(item.id as BackgroundCommand);
				}
				if (!model) return;
				const current = model.sort;
				const group = groupFromMenuId(item.id);
				if (group !== null) {
					if (group !== current.groupBy) void model.setSort({ ...current, groupBy: group });
				} else if (item.id.startsWith('sort:')) {
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
