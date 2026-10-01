// Subscribes to the shell's request to report tab hints before the window closes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

/** The event the shell sends this window when it is about to close (see `persistence.rs`). */
export const FLUSH_HINTS_EVENT = 'waypoint://flush-hints';

/** Calls `flush` for each request and returns the function that stops listening; a no-op outside the shell. */
export function onFlushHints(flush: () => void): () => void {
	if (!isTauri()) return () => {};
	let stopped = false;
	let unlisten: (() => void) | null = null;
	getCurrentWebviewWindow()
		.listen(FLUSH_HINTS_EVENT, flush)
		.then((stop) => {
			if (stopped) stop();
			else unlisten = stop;
		})
		.catch((error: unknown) => console.warn('could not listen for hint flushes', error));
	return () => {
		stopped = true;
		unlisten?.();
	};
}
