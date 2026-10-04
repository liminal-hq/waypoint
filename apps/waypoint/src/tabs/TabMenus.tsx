// The tab's right-click menu and the + button's menu, over the shared context menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { t } from '../i18n/messages';
import { useGroupActions } from './groupActions';
import { useTabsSnapshot } from './TabsContext';
import { useTabActions } from './tabActions';
import { useTabExtras } from './tabExtras';
import { useOtherWindows } from './useOtherWindows';
import { useWindowActions } from './windowActions';
import { usePairActions } from './pairActions';
import { pairOfTab } from './pairLayout';
import { insertBefore, pairTabItems, runPairMenuItem } from './pairMenus';
import { useConnections } from '../connections/ConnectionsContext';
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
	const groupActions = useGroupActions();
	const pairActions = usePairActions();
	const snapshot = useTabsSnapshot();
	const groups = snapshot?.groups ?? [];
	const pairs = snapshot?.pairs ?? [];
	const pair = pairOfTab(pairs, tab.id);
	const windowActions = useWindowActions();
	const others = useOtherWindows(windowActions);
	return (
		<ContextMenu
			{...rest}
			ariaLabel={t('tabs.menu.label')}
			items={insertBefore(
				tabMenuItems(tab, closed, groups, others),
				'sep-close',
				pairTabItems(tab, pair, snapshot?.tabs ?? [], pairs),
			)}
			onSelect={(item) => {
				if (runPairMenuItem(item, { actions: pairActions, pair, tab })) return;
				runTabMenuItem(item, tab, actions, {
					closed,
					extras,
					groups,
					groupActions,
					windows: { others, actions: windowActions },
				});
			}}
		/>
	);
}

/** Opened by press-and-hold or right-click on the + button, or by the Menu key when it has focus. */
export function PlusMenu({ closed, ...rest }: MenuProps) {
	const connect = useConnections() !== null;
	const actions = useTabActions();
	const extras = useTabExtras();
	const windows = useWindowActions();
	return (
		<ContextMenu
			{...rest}
			ariaLabel={t('tabs.plusMenu.label')}
			items={plusMenuItems(closed, connect)}
			onSelect={(item) => runPlusMenuItem(item, actions, { closed, extras, windows })}
		/>
	);
}
