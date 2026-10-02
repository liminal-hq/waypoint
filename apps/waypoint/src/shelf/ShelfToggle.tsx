// The button that shows and hides the Shelf, and the dock it shows in
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useStore } from 'zustand';
import { tf, t } from '../i18n/messages';
import { ShelfIcon } from './ShelfIcons';
import { ShelfPanel } from './ShelfPanel';
import { useShelfStore } from './shelfStore';
import styles from './ShelfToggle.module.css';

/** A toggle for the status bar's end: pressed while the Shelf shows, with the number of items it holds. */
export function ShelfToggle() {
	const store = useShelfStore();
	const open = useStore(store, (s) => s.open);
	const count = useStore(store, (s) => s.items.length);
	return (
		<button
			type="button"
			className={styles.button}
			aria-pressed={open}
			aria-label={t('shelf.toggle')}
			title={tf('view.withShortcut', { name: t('shelf.toggle'), keys: 'Ctrl+B' })}
			onClick={() => store.getState().toggleOpen()}
		>
			<ShelfIcon />
			{count > 0 && <span className={styles.badge}>{count}</span>}
		</button>
	);
}

/** The dock along the bottom of the window, between the panes and the status bar, while the Shelf is open. */
export function ShelfDock() {
	const store = useShelfStore();
	const open = useStore(store, (s) => s.open);
	return open ? <ShelfPanel /> : null;
}
