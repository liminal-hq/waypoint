// The header over a pane of a pair: its folder, whether it is the active pane, and a close button
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { PointerEvent } from 'react';
import { t, tf } from '../i18n/messages';
import { CloseSmallIcon } from '../icons/AppIcons';
import { FileIcon } from '../browse/FileIcon';
import { GripIcon } from './PairIcons';
import { useTabTitle } from './tabTitle';
import styles from './PaneHeader.module.css';

interface PaneHeaderProps {
	tab: TabSnapshot;
	active: boolean;
	onClose: () => void;
	/** A press on the grip may become a drag up to the strip, which separates the pair. */
	onGrip?: (event: PointerEvent<HTMLElement>) => void;
}

/**
 * Shown only when the tab is paired (a single tab shows no header). The grip is where a pane is
 * grabbed: dragging it up to the tab strip separates the pair (Separate Tabs is the keyboard
 * route); tearing one half off into a window arrives with tear-off. Pressing anywhere on the header activates the pane (the pane
 * area does that), and the close button closes this pane's tab, leaving the other as a single tab.
 */
export function PaneHeader({ tab, active, onClose, onGrip }: PaneHeaderProps) {
	const title = useTabTitle(tab);
	return (
		<div className={styles.header} data-active={active ? '' : undefined}>
			<span
				className={styles.gripHandle}
				data-pane-grip=""
				title={t('pair.pane.grip')}
				onPointerDown={onGrip}
			>
				<GripIcon className={styles.grip} width={12} height={12} />
			</span>
			<FileIcon group="folder" className={styles.icon} />
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
