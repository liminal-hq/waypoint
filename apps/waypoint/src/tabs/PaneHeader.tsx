// The header over a pane of a pair: its folder, whether it is the active pane, and a close button
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { t, tf } from '../i18n/messages';
import { CloseSmallIcon, FolderTabIcon } from '../icons/AppIcons';
import { GripIcon } from './PairIcons';
import { useTabTitle } from './tabTitle';
import styles from './PaneHeader.module.css';

interface PaneHeaderProps {
	tab: TabSnapshot;
	active: boolean;
	onClose: () => void;
}

/**
 * Shown only when the tab is paired (a single tab shows no header). The grip is where a pane is
 * grabbed to separate it or tear it off; dragging it arrives with the tab drag engine, so for now
 * it is a mark and nothing more. Pressing anywhere on the header activates the pane (the pane
 * area does that), and the close button closes this pane's tab, leaving the other as a single tab.
 */
export function PaneHeader({ tab, active, onClose }: PaneHeaderProps) {
	const title = useTabTitle(tab);
	return (
		<div className={styles.header} data-active={active ? '' : undefined}>
			<GripIcon className={styles.grip} width={12} height={12} data-pane-grip="" />
			<FolderTabIcon className={styles.icon} width={14} height={14} />
			<span className={styles.title} title={tab.location.display}>
				{title}
			</span>
			{active ? <span className={styles.badge}>{t('pair.pane.active')}</span> : null}
			<button
				type="button"
				className={styles.close}
				aria-label={tf('pair.pane.close', { title })}
				title={tf('pair.pane.close', { title })}
				onClick={onClose}
			>
				<CloseSmallIcon width={12} height={12} />
			</button>
		</div>
	);
}
