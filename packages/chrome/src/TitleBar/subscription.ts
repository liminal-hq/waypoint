// Adapts asynchronous listener registration to the chrome's synchronous `Unsubscribe`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Unsubscribe } from './windowControls';

/**
 * Wraps Tauri's asynchronous listener registration as an `Unsubscribe` whose `ready` resolves once
 * the listener is live, and which still cleans up if it is cancelled before that happens.
 */
export function subscription(register: () => Promise<() => void>): Unsubscribe {
	let disposed = false;
	let unlisten: (() => void) | undefined;
	const ready = register()
		.then((off) => {
			if (disposed) off();
			else unlisten = off;
		})
		.catch(() => {});
	const unsubscribe: Unsubscribe = () => {
		disposed = true;
		unlisten?.();
	};
	unsubscribe.ready = ready;
	return unsubscribe;
}
