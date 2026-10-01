// The window items of the tab menu: Move to New Window and Move to Window ▸, and what they do
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem, SelectableMenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { t } from '../i18n/messages';
import { windowMenuLabel, type WindowActions } from './windowActions';

const WINDOW_PREFIX = 'window:';

/** Move to New Window, and Move to Window ▸ listing the other windows (disabled when there are none). */
export function windowMoveItems(others: readonly WindowSummary[]): MenuItem[] {
	return [
		{ type: 'action', id: 'moveToNewWindow', label: t('tabs.menu.moveToNewWindow') },
		{
			type: 'submenu',
			id: 'moveToWindow',
			label: t('tabs.menu.moveToWindow'),
			disabled: others.length === 0,
			items: others.map((window) => ({
				type: 'action' as const,
				id: `${WINDOW_PREFIX}${window.label}`,
				label: windowMenuLabel(window),
			})),
		},
	];
}

/** Does what choosing `item` means for `tab` when it is a window item, and says whether it was one. */
export function runWindowMoveItem(
	item: SelectableMenuItem,
	tab: TabSnapshot,
	others: readonly WindowSummary[],
	actions: Pick<WindowActions, 'moveToNewWindow' | 'moveToWindow'>,
): boolean {
	if (item.id === 'moveToNewWindow') {
		void actions.moveToNewWindow(tab);
		return true;
	}
	if (item.id.startsWith(WINDOW_PREFIX)) {
		const target = others.find((window) => `${WINDOW_PREFIX}${window.label}` === item.id);
		if (target) void actions.moveToWindow(tab, target);
		return true;
	}
	return false;
}
