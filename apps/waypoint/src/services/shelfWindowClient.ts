// The page's view of the Shelf window: raise it, hide it and follow whether it is on screen
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Unsubscribe } from './vfsClient';

/**
 * What a page may ask of the Shelf window, which exists while the Shelf is undocked. Making and
 * closing it is the session's (`TabsApi.setShelfUndocked`); this is only whether it is shown.
 */
export interface ShelfWindowClient {
	/** Raises the Shelf window, or hides it when it is on screen and focused. Resolves to whether it is shown afterwards. */
	toggle(): Promise<boolean>;
	/** Brings the Shelf window to the front. Resolves to whether there was one. */
	raise(): Promise<boolean>;
	/** Whether the Shelf window is on screen right now. */
	visible(): Promise<boolean>;
	/** Follows the window being shown or hidden. */
	onVisibleChange(listener: (shown: boolean) => void): Unsubscribe;
}
