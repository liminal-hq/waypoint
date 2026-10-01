// The one label that follows the pointer during a drag and says what a release would do
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createPortal } from 'react-dom';
import { t } from '../i18n/messages';
import { useSeparateSession, useTabDragState } from './TabDragContext';
import { useStore } from 'zustand';
import styles from './TabDragPill.module.css';

/**
 * A small floating label near the pointer: what a release would do, and “Esc to cancel”. It is
 * drawn, not announced: the live region says the same text (`role="status"` here would read it a
 * second time). It follows the pointer through `--wp-drag-x` and `--wp-drag-y`, which the drag
 * session writes, so moving it does not render React.
 */
export function TabDragPill() {
	const tab = useTabDragState();
	const separate = useStore(useSeparateSession().store);
	// Only one drag runs at a time, so at most one of the two sessions has a pill.
	const { phase, pill } = tab.phase === 'dragging' ? tab : separate;
	if (phase !== 'dragging' || !pill || pill.undrawn) return null;
	return createPortal(
		<div className={styles.pill} data-kind={pill.kind} aria-hidden="true" data-drag-pill="">
			<span className={styles.text}>{pill.text}</span>
			<span className={styles.hint}>{t('drag.pill.esc')}</span>
		</div>,
		document.body,
	);
}
