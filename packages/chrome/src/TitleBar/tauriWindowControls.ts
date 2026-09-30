// Tauri implementation of the WindowControls adapter
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Unsubscribe, WindowControls } from './windowControls';

/**
 * Wraps Tauri's asynchronous listener registration as an `Unsubscribe` whose `ready` resolves once
 * the listener is live, and which still cleans up if it is cancelled before that happens.
 */
function subscription(register: () => Promise<() => void>): Unsubscribe {
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

/** Creates an adapter bound to the current Tauri window. Resolves the window lazily on each call. */
export function createTauriWindowControls(): WindowControls {
	return {
		minimize: () => getCurrentWindow().minimize(),
		toggleMaximize: () => getCurrentWindow().toggleMaximize(),
		close: () => getCurrentWindow().close(),
		startDragging: () => getCurrentWindow().startDragging(),
		setAlwaysOnTop: (value) => getCurrentWindow().setAlwaysOnTop(value),
		isAlwaysOnTop: () => getCurrentWindow().isAlwaysOnTop(),
		isMaximized: () => getCurrentWindow().isMaximized(),
		onMaximizedChange(listener) {
			const win = getCurrentWindow();
			// Each resize event reads the state asynchronously; only the latest read may report, so an
			// older read that resolves late cannot overwrite a newer one.
			let latest = 0;
			return subscription(() =>
				win.onResized(async () => {
					const mine = ++latest;
					const maximised = await win.isMaximized();
					if (mine === latest) listener(maximised);
				}),
			);
		},
		isFocused: () => getCurrentWindow().isFocused(),
		onFocusChange(listener) {
			return subscription(() =>
				getCurrentWindow().onFocusChanged(({ payload }) => listener(payload)),
			);
		},
		// Tauri's drag-region script already toggles maximise on double-click.
		handlesDoubleClickNatively: true,
	};
}

/** Shared instance for the common single-window case. */
export const tauriWindowControls: WindowControls = createTauriWindowControls();
