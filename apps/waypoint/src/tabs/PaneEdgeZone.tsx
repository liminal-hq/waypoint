// The highlight over the file area's edge that a dragged tab would split the pane on
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useTabDragState } from './TabDragContext';
import styles from './PaneEdgeZone.module.css';

/**
 * Drawn inside the pane area while a tab is dragged over one of its edge zones: the half of the
 * area the dragged tab's pane would take. It is a mark, not a control: the zones themselves were
 * measured once when the drag began and the engine decides which one is in play.
 */
export function PaneEdgeZone() {
	const { phase, target } = useTabDragState();
	if (phase !== 'dragging' || target?.outcome !== 'splitPane') return null;
	return <div className={styles.zone} data-edge={target.edge} aria-hidden="true" />;
}
