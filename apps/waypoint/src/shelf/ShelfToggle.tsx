// The button that shows and hides the Shelf, and the dock it shows in
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useStore } from 'zustand';
import { tf, t } from '../i18n/messages';
import { ShelfIcon } from './ShelfIcons';
import { useShelfPlacement } from './ShelfContext';
import { ShelfPanel } from './ShelfPanel';
import { useShelfStore } from './shelfStore';
import styles from './ShelfToggle.module.css';

/**
 * A toggle for the status bar's end: pressed while the Shelf shows, with the number of items it
 * holds. With the Shelf undocked it raises or hides the Shelf window, and is pressed while that
 * window is on screen.
 */
export function ShelfToggle() {
	const store = useShelfStore();
	const placement = useShelfPlacement();
	const undocked = useStore(store, (s) => s.undocked);
	const shown = useStore(store, (s) => (s.undocked ? s.windowShown : s.open));
	const count = useStore(store, (s) => s.items.length);
	return (
		<button
			type="button"
			className={styles.button}
			aria-pressed={shown}
			aria-label={t('shelf.toggle')}
			title={tf('view.withShortcut', {
				name: undocked ? t('shelf.toggle.window') : t('shelf.toggle'),
				keys: 'Ctrl+B',
			})}
			onClick={() => (placement ? placement.toggle() : store.getState().toggleOpen())}
		>
			<ShelfIcon />
			{count > 0 && <span className={styles.badge}>{count}</span>}
		</button>
	);
}

/**
 * The dock along the bottom of the content column, under the pane or panes and beside the sidebar,
 * while the Shelf is open and docked. While the Shelf is undocked into its own window no main window keeps a dock.
 */
export function ShelfDock() {
	const store = useShelfStore();
	const show = useStore(store, (s) => s.open && !s.undocked);
	return show ? <ShelfPanel /> : null;
}
