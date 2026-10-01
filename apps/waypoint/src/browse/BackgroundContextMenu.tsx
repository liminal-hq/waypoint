// The context menu of the file area's empty space: sort and hidden files
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { SortKey } from '@liminal-hq/waypoint-protocol/generated/SortKey';
import { t, type MessageId } from '../i18n/messages';
import type { ListingSession } from './useListingSession';

const SORT_KEYS: Array<{ key: SortKey; label: MessageId }> = [
	{ key: 'name', label: 'menu.sort.name' },
	{ key: 'size', label: 'menu.sort.size' },
	{ key: 'modified', label: 'menu.sort.modified' },
	{ key: 'kind', label: 'menu.sort.kind' },
];

interface BackgroundContextMenuProps {
	session: ListingSession | null;
	showHidden: boolean;
	position: { x: number; y: number };
	keyboard: boolean;
	onToggleHidden: () => void;
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
	onClose,
}: BackgroundContextMenuProps) {
	const sort = session?.model.sort;
	const items: MenuItem[] = [
		...(sort
			? ([
					{ type: 'section', label: t('menu.sortBy') },
					...SORT_KEYS.map(({ key, label }): MenuItem => ({
						type: 'checkbox',
						id: `sort:${key}`,
						label: t(label),
						checked: sort.key === key,
					})),
					{ type: 'separator' },
					{
						type: 'checkbox',
						id: 'descending',
						label: t('menu.sort.descending'),
						checked: sort.descending,
					},
					{
						type: 'checkbox',
						id: 'foldersFirst',
						label: t('menu.sort.foldersFirst'),
						checked: sort.directoriesFirst,
					},
					{ type: 'separator' },
				] as MenuItem[])
			: []),
		{
			type: 'checkbox',
			id: 'showHidden',
			label: t('menu.showHidden'),
			checked: showHidden,
			shortcut: 'Ctrl+H',
		},
	];

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
