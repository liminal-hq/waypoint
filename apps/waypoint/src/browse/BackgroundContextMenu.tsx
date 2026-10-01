// The context menu of the file area's empty space: sort and hidden files
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
	SizeIcon,
	TagIcon,
	TextIcon,
	TrashIcon,
} from '../icons/MenuIcons';
import { t, type MessageId } from '../i18n/messages';
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

/**
 * The empty-space menu's items: sort (when a listing is open) and the hidden-files toggle. In the
 * Trash the sort keys are Name, Size and Date deleted, there is no hidden-files toggle (nothing is
 * hidden), and Empty Trash comes last.
 */
export function backgroundMenuItems(
	sort: SortSpec | undefined,
	showHidden: boolean,
	trash: TrashBackground | null = null,
): MenuItem[] {
	const keys = SORT_KEYS.filter(({ key }) =>
		trash ? TRASH_SORT_KEYS.includes(key) : key !== 'deleted',
	);
	const items: MenuItem[] = [
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
}: BackgroundContextMenuProps) {
	const inTrash = session?.model.layout === 'trash';
	const items = backgroundMenuItems(
		session?.model.sort,
		showHidden,
		inTrash ? { count: session.model.count } : null,
	);

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
