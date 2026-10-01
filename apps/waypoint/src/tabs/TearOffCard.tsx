// The mini window card that follows the pointer inside this window during a new-window drag
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import { useStore } from 'zustand';
import { TearCard } from '../app/TearCard';
import { CARD_SIZE } from './tearOffPlacement';
import type { TearCardStore } from './tearOffCard';
import styles from './TearOffCard.module.css';

/**
 * The preview of the window a release would open, drawn in the page wherever the pointer is over
 * this window (the plugin's ghost takes over outside it, where the page cannot draw). It follows
 * `--wp-drag-x` and `--wp-drag-y`, which the drag session writes, so moving it does not render
 * React. It is decoration: the pill and the live region say what a release does.
 */
export function TearOffCard({ store }: { store: TearCardStore }) {
	const payload = useStore(store, (state) => state.payload);
	if (!payload) return null;
	return createPortal(
		<div
			className={styles.card}
			aria-hidden="true"
			data-tear-off-card=""
			style={
				{
					'--wp-tear-card-width': `${CARD_SIZE.width}px`,
					'--wp-tear-card-height': `${CARD_SIZE.height}px`,
				} as CSSProperties
			}
		>
			<TearCard payload={payload} />
		</div>,
		document.body,
	);
}
