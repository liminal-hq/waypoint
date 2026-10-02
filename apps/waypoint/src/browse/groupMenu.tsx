// The empty-space menu's Group by section: a checked list of what the listing is divided by, with No grouping first
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { GroupBy } from '@liminal-hq/waypoint-protocol/generated/GroupBy';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import type { ReactNode } from 'react';
import {
	ClockIcon,
	NewFileIcon,
	SizeIcon,
	TagIcon,
	TextIcon,
	UngroupIcon,
} from '../icons/MenuIcons';
import { t, type MessageId } from '../i18n/messages';

/** The full name of each grouping, as the commands and screen readers say it. */
const GROUP_COMMAND_LABELS: Record<GroupBy, MessageId> = {
	none: 'cmd.group.none',
	kind: 'cmd.group.kind',
	modified: 'cmd.group.modified',
	size: 'cmd.group.size',
	name: 'cmd.group.name',
	type: 'cmd.group.type',
};

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
 * Group by, as a section of its own: a heading and checked items (the one in use is checked), ending
 * in a separator. The short labels repeat the Sort by ones, so each carries a full name for a
 * screen reader ("Group by Name").
 */
export function groupByItems(sort: SortSpec): MenuItem[] {
	return [
		{ type: 'section', label: t('menu.groupBy') },
		...GROUPS.map(({ by, label, icon }): MenuItem => ({
			type: 'checkbox',
			id: `${PREFIX}${by}`,
			label: t(label),
			ariaLabel: t(GROUP_COMMAND_LABELS[by]),
			icon,
			checked: sort.groupBy === by,
		})),
		{ type: 'separator' },
	];
}

/** The grouping a menu item chose, or `null` for an item that is not one of these. */
export function groupFromMenuId(id: string): GroupBy | null {
	if (!id.startsWith(PREFIX)) return null;
	const by = id.slice(PREFIX.length);
	return GROUPS.find((group) => group.by === by)?.by ?? null;
}
