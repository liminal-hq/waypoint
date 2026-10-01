// The pair joint's right-click menu, over the shared context menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { useEffect, useState } from 'react';
import { t } from '../i18n/messages';
import { usePairActions } from './pairActions';
import { pairJointItems, runPairMenuItem } from './pairMenus';
import { useTabsSnapshot } from './TabsContext';
import { useWindowActions } from './windowActions';

interface PairJointMenuProps {
	pair: Pair;
	position: MenuPosition;
	openedWithKeyboard: boolean;
	/** What takes the focus back when the menu closes. */
	returnFocusTo: HTMLElement | null;
	onClose: () => void;
}

/** Opened by right-click, the Menu key or Shift+F10 on the pair's joint. */
export function PairJointMenu({ pair, ...rest }: PairJointMenuProps) {
	const actions = usePairActions();
	const tabs = useTabsSnapshot()?.tabs ?? [];
	const windows = useWindowActions();
	const [others, setOthers] = useState<WindowSummary[] | null>(null);
	// The other windows are read fresh when the menu opens: they come and go unheard.
	useEffect(() => {
		let current = true;
		void windows.otherWindows().then((list) => {
			if (current) setOthers(list);
		});
		return () => {
			current = false;
		};
	}, [windows]);
	if (others === null) return null;
	return (
		<ContextMenu
			{...rest}
			ariaLabel={t('pair.menu.label')}
			items={pairJointItems(pair, tabs, [], others)}
			onSelect={(item) => runPairMenuItem(item, { actions, pair, others })}
		/>
	);
}
