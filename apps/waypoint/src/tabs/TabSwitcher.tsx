// The overlay shown while Ctrl is held during Ctrl+Tab: the tabs in walk order, the candidate highlighted
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { createPortal } from 'react-dom';
import { t } from '../i18n/messages';
import { FolderTabIcon, PinIcon } from '../icons/AppIcons';
import { useSwitcherWalk } from './mruSwitcher';
import { useTabsSnapshot } from './TabsContext';
import { useTabTitle } from './tabTitle';
import styles from './TabSwitcher.module.css';

/**
 * Purely visual: focus stays where it was and the candidate is announced through the strip's
 * live region, so the overlay is hidden from the accessibility tree.
 */
export function TabSwitcher() {
	const walk = useSwitcherWalk();
	const snapshot = useTabsSnapshot();
	if (!walk || !snapshot) return null;
	return createPortal(
		<div className={styles.overlay} aria-hidden="true" data-testid="tab-switcher">
			<ul className={styles.list} aria-label={t('tabs.switcher.label')}>
				{walk.order.map((id, position) => {
					const tab = snapshot.tabs.find((candidate) => candidate.id === id);
					return tab ? (
						<SwitcherRow key={id} tab={tab} candidate={position === walk.index} />
					) : null;
				})}
			</ul>
		</div>,
		document.body,
	);
}

function SwitcherRow({ tab, candidate }: { tab: TabSnapshot; candidate: boolean }) {
	const title = useTabTitle(tab);
	return (
		<li className={styles.row} data-candidate={candidate ? '' : undefined}>
			<FolderTabIcon className={styles.icon} />
			<span className={styles.title}>{title}</span>
			{tab.pinned ? <PinIcon className={styles.pin} width={12} height={12} /> : null}
		</li>
	);
}
