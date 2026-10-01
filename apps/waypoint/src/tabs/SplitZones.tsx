// The four labelled split regions drawn over the file area while a dragged tab is over it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, type MessageId } from '../i18n/messages';
import type { Edge } from './dragLayout';
import { SPLIT_EDGES } from './splitRegions';
import { useTabDragState } from './TabDragContext';
import styles from './SplitZones.module.css';

const LABELS = {
	left: 'drag.zone.left',
	right: 'drag.zone.right',
	top: 'drag.zone.top',
	bottom: 'drag.zone.bottom',
} as const satisfies Record<Edge, MessageId>;

/**
 * Drawn inside the pane area while the engine has a `splitPane` target: all four regions with
 * their labels, the one under the pointer solid, and the half of the area the new pane would take
 * tinted. It is a mark, not a control: the regions were measured once when the drag began and the
 * engine decides which one is in play, so it takes no pointer events and is hidden from assistive
 * technology (the drag pill and the live region say the same in words).
 */
export function SplitZones() {
	const { phase, target } = useTabDragState();
	if (phase !== 'dragging' || target?.outcome !== 'splitPane') return null;
	return (
		<div className={styles.zones} aria-hidden="true" data-split-zones="" data-edge={target.edge}>
			<div className={styles.preview} data-edge={target.edge} />
			{SPLIT_EDGES.map((edge) => (
				<div
					key={edge}
					className={styles.zone}
					data-zone={edge}
					data-hovered={edge === target.edge ? '' : undefined}
				>
					<span className={styles.label}>{t(LABELS[edge])}</span>
				</div>
			))}
		</div>
	);
}
