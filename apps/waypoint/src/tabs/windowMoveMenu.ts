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

/** The two item labels, which say what is moving ("Move Group to New Window"). */
export interface WindowMoveLabels {
	newWindow: string;
	toWindow: string;
}

/** Move to New Window, and Move to Window ▸ listing the other windows (disabled when there are none). */
export function windowMoveItems(
	others: readonly WindowSummary[],
	labels: WindowMoveLabels = {
		newWindow: t('tabs.menu.moveToNewWindow'),
		toWindow: t('tabs.menu.moveToWindow'),
	},
): MenuItem[] {
	return [
		{ type: 'action', id: 'moveToNewWindow', label: labels.newWindow },
		{
			type: 'submenu',
			id: 'moveToWindow',
			label: labels.toWindow,
			disabled: others.length === 0,
			items: others.map((window) => ({
				type: 'action' as const,
				id: `${WINDOW_PREFIX}${window.label}`,
				label: windowMenuLabel(window),
			})),
		},
	];
}

/** What moving does, whatever is moving: to a window of its own, or to the end of another one. */
export interface WindowMoveChoice {
	toNewWindow(): void;
	toWindow(target: WindowSummary): void;
}

/** Does what choosing `item` means when it is a window item, and says whether it was one. */
export function runWindowMoveChoice(
	item: SelectableMenuItem,
	others: readonly WindowSummary[],
	choice: WindowMoveChoice,
): boolean {
	if (item.id === 'moveToNewWindow') {
		choice.toNewWindow();
		return true;
	}
	if (item.id.startsWith(WINDOW_PREFIX)) {
		const target = others.find((window) => `${WINDOW_PREFIX}${window.label}` === item.id);
		if (target) choice.toWindow(target);
		return true;
	}
	return false;
}

/** Does what choosing `item` means for `tab` when it is a window item, and says whether it was one. */
export function runWindowMoveItem(
	item: SelectableMenuItem,
	tab: TabSnapshot,
	others: readonly WindowSummary[],
	actions: Pick<WindowActions, 'moveToNewWindow' | 'moveToWindow'>,
): boolean {
	return runWindowMoveChoice(item, others, {
		toNewWindow: () => void actions.moveToNewWindow(tab),
		toWindow: (target) => void actions.moveToWindow(tab, target),
	});
}
