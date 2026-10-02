// The real ShelfWindowClient: the app's Shelf window commands and its visibility event
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ShelfWindowClient } from './shelfWindowClient';

/** Mirrors `VISIBLE_EVENT` in `src-tauri/src/shelf_window.rs`. */
export const SHELF_VISIBLE_EVENT = 'waypoint://shelf-window-visible';

export function createTauriShelfWindowClient(): ShelfWindowClient {
	return {
		toggle: () => invoke<boolean>('toggle_shelf_window'),
		raise: () => invoke<boolean>('raise_shelf_window'),
		visible: () => invoke<boolean>('shelf_window_visible'),
		onVisibleChange(listener) {
			let stopped = false;
			let unlisten: (() => void) | undefined;
			listen<boolean>(SHELF_VISIBLE_EVENT, (event) => listener(event.payload)).then(
				(stop) => {
					if (stopped) stop();
					else unlisten = stop;
				},
				(error: unknown) => console.warn('could not follow the Shelf window', error),
			);
			return () => {
				stopped = true;
				unlisten?.();
				unlisten = undefined;
			};
		},
	};
}
