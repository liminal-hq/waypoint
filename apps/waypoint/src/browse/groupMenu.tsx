// The empty-space menu's Group by block: a checked list of what the listing is divided by, with No grouping first
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { GroupBy } from '@liminal-hq/waypoint-protocol/generated/GroupBy';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import type { ReactNode } from 'react';
import {
	ClockIcon,
	GroupIcon,
	NewFileIcon,
	SizeIcon,
	TagIcon,
	TextIcon,
	UngroupIcon,
} from '../icons/MenuIcons';
import { t, type MessageId } from '../i18n/messages';

const GROUPS: Array<{ by: GroupBy; label: MessageId; icon: ReactNode }> = [
	{ by: 'none', label: 'menu.group.none', icon: <UngroupIcon /> },
	{ by: 'kind', label: 'menu.group.kind', icon: <TagIcon /> },
	{ by: 'modified', label: 'menu.group.modified', icon: <ClockIcon /> },
	{ by: 'size', label: 'menu.group.size', icon: <SizeIcon /> },
	{ by: 'name', label: 'menu.group.name', icon: <TextIcon /> },
	{ by: 'type', label: 'menu.group.type', icon: <NewFileIcon /> },
];

const PREFIX = 'group:';

/**
 * Group by, as a submenu of checked items (the one in use is checked), ending in a separator. A
 * submenu keeps the menu short, and keeps "Name" and "Size" meaning one item each in it.
 */
export function groupByItems(sort: SortSpec): MenuItem[] {
	return [
		{
			type: 'submenu',
			id: 'groupBy',
			label: t('menu.groupBy'),
			icon: <GroupIcon />,
			items: GROUPS.map(({ by, label, icon }): MenuItem => ({
				type: 'checkbox',
				id: `${PREFIX}${by}`,
				label: t(label),
				icon,
				checked: sort.groupBy === by,
			})),
		},
		{ type: 'separator' },
	];
}

/** The grouping a menu item chose, or `null` for an item that is not one of these. */
export function groupFromMenuId(id: string): GroupBy | null {
	if (!id.startsWith(PREFIX)) return null;
	const by = id.slice(PREFIX.length);
	return GROUPS.find((group) => group.by === by)?.by ?? null;
}
