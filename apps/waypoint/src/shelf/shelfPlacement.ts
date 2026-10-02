// Where the Shelf is shown: docked in each window, or in the one Shelf window, and the moves between the two
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import type { ShelfWindowClient } from '../services/shelfWindowClient';
import type { TabsApi } from '../services/tabsApi';
import type { ShelfStore } from './shelfStore';

export interface ShelfPlacementDeps {
	store: ShelfStore;
	api: Pick<TabsApi, 'setShelfUndocked'>;
	client: ShelfWindowClient;
	/** This page is the Shelf window itself, not a main window with a dock. */
	inWindow: boolean;
	say(text: string): void;
	announce(text: string): void;
}

export interface ShelfPlacement {
	/**
	 * Ctrl+B and the status bar button. Docked, it shows or hides the dock in this window; with the
	 * Shelf undocked, it raises the Shelf window, or hides it when it is on screen and focused.
	 */
	toggle(): void;
	/** Moves the keyboard into the Shelf, raising its window when it has one. */
	focus(): void;
	/** Puts the Shelf in its own window. The Shelf stays docked when the window cannot be made. */
	undock(): Promise<void>;
	/** Puts the Shelf back in the main windows' docks, which closes the Shelf window. */
	dock(): Promise<void>;
}

export function createShelfPlacement(deps: ShelfPlacementDeps): ShelfPlacement {
	const { store, api, client, inWindow, say, announce } = deps;
	const windowFailed = (error: unknown) => {
		console.warn('could not show or hide the Shelf window', error);
		say(t('shelf.window.failed'));
	};
	return {
		toggle() {
			if (inWindow || store.getState().undocked) {
				client.toggle().catch(windowFailed);
			} else {
				store.getState().toggleOpen();
			}
		},
		focus() {
			if (inWindow) {
				store.getState().requestFocus();
			} else if (store.getState().undocked) {
				client.raise().catch(windowFailed);
			} else {
				store.getState().setOpen(true);
				store.getState().requestFocus();
			}
		},
		async undock() {
			if (store.getState().undocked) return;
			try {
				await api.setShelfUndocked(true);
				announce(t('shelf.undocked'));
			} catch (error) {
				console.warn('could not undock the Shelf', error);
				say(t('shelf.undock.failed'));
			}
		},
		async dock() {
			if (!store.getState().undocked) return;
			try {
				await api.setShelfUndocked(false);
				announce(t('shelf.docked'));
			} catch (error) {
				console.warn('could not dock the Shelf', error);
				say(t('shelf.dock.failed'));
			}
		},
	};
}
