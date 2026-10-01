// The tab's right-click menu and the + button's menu, over the shared context menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { useEffect, useState } from 'react';
import { t } from '../i18n/messages';
import { useTabActions } from './tabActions';
import { useTabExtras } from './tabExtras';
import { useWindowActions } from './windowActions';
import { plusMenuItems, runPlusMenuItem, runTabMenuItem, tabMenuItems } from './tabMenuModel';

interface MenuProps {
	position: MenuPosition;
	closed: ClosedTab[];
	openedWithKeyboard: boolean;
	/** What takes the focus back when the menu closes. */
	returnFocusTo: HTMLElement | null;
	onClose: () => void;
}

/**
 * Opened by right-click, or by the Menu key or Shift+F10 on a focused tab. The other windows are
 * read fresh when it opens, because windows come and go without this one hearing of it.
 */
export function TabContextMenu({ tab, closed, ...rest }: MenuProps & { tab: TabSnapshot }) {
	const actions = useTabActions();
	const extras = useTabExtras();
	const windowActions = useWindowActions();
	const [others, setOthers] = useState<WindowSummary[] | null>(null);
	useEffect(() => {
		let current = true;
		void windowActions.otherWindows().then((windows) => {
			if (current) setOthers(windows);
		});
		return () => {
			current = false;
		};
	}, [windowActions]);
	if (others === null) return null;
	return (
		<ContextMenu
			{...rest}
			ariaLabel={t('tabs.menu.label')}
			items={tabMenuItems(tab, closed, others)}
			onSelect={(item) =>
				runTabMenuItem(item, tab, actions, {
					closed,
					extras,
					windows: { others, actions: windowActions },
				})
			}
		/>
	);
}

/** Opened by press-and-hold or right-click on the + button, or by the Menu key when it has focus. */
export function PlusMenu({ closed, ...rest }: MenuProps) {
	const actions = useTabActions();
	const extras = useTabExtras();
	const windows = useWindowActions();
	return (
		<ContextMenu
			{...rest}
			ariaLabel={t('tabs.plusMenu.label')}
			items={plusMenuItems(closed)}
			onSelect={(item) => runPlusMenuItem(item, actions, { closed, extras, windows })}
		/>
	);
}
