// The right-click menu of a tab: where it can go. Other tab commands join it in later slices.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type {
	MenuItem,
	MenuPosition,
	SelectableMenuItem,
} from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { useEffect, useState } from 'react';
import { t } from '../i18n/messages';
import { useWindowActions, windowMenuLabel, type WindowActions } from './windowActions';

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

/** What choosing `item` does to `tab`. */
export function runWindowMoveItem(
	item: SelectableMenuItem,
	tab: TabSnapshot,
	others: readonly WindowSummary[],
	actions: Pick<WindowActions, 'moveToNewWindow' | 'moveToWindow'>,
): void {
	if (item.id === 'moveToNewWindow') {
		void actions.moveToNewWindow(tab);
	} else if (item.id.startsWith(WINDOW_PREFIX)) {
		const target = others.find((window) => `${WINDOW_PREFIX}${window.label}` === item.id);
		if (target) void actions.moveToWindow(tab, target);
	}
}

interface TabContextMenuProps {
	tab: TabSnapshot;
	position: MenuPosition;
	/** Opened with the Menu key or Shift+F10, so focus goes into the menu. */
	openedWithKeyboard: boolean;
	/** What takes the focus back when the menu closes. */
	returnFocusTo: HTMLElement | null;
	onClose: () => void;
}

/**
 * Opened by right-click, or by the Menu key or Shift+F10 on a focused tab. The other windows are
 * read fresh when it opens, because windows come and go without this one hearing of it.
 */
export function TabContextMenu({
	tab,
	position,
	openedWithKeyboard,
	returnFocusTo,
	onClose,
}: TabContextMenuProps) {
	const actions = useWindowActions();
	const [others, setOthers] = useState<WindowSummary[] | null>(null);
	useEffect(() => {
		let current = true;
		void actions.otherWindows().then((windows) => {
			if (current) setOthers(windows);
		});
		return () => {
			current = false;
		};
	}, [actions]);
	if (others === null) return null;
	return (
		<ContextMenu
			position={position}
			ariaLabel={t('tabs.menu.label')}
			openedWithKeyboard={openedWithKeyboard}
			returnFocusTo={returnFocusTo}
			items={windowMoveItems(others)}
			onClose={onClose}
			onSelect={(item) => {
				onClose();
				runWindowMoveItem(item, tab, others, actions);
			}}
		/>
	);
}
