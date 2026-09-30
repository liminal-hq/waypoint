// Tauri implementation of the WindowControls adapter
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getCurrentWindow } from '@tauri-apps/api/window';
import type { WindowControls } from './windowControls';

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
			let disposed = false;
			let unlisten: (() => void) | undefined;
			void win
				.onResized(async () => listener(await win.isMaximized()))
				.then((off) => {
					if (disposed) off();
					else unlisten = off;
				});
			return () => {
				disposed = true;
				unlisten?.();
			};
		},
		isFocused: () => getCurrentWindow().isFocused(),
		onFocusChange(listener) {
			let disposed = false;
			let unlisten: (() => void) | undefined;
			void getCurrentWindow()
				.onFocusChanged(({ payload }) => listener(payload))
				.then((off) => {
					if (disposed) off();
					else unlisten = off;
				});
			return () => {
				disposed = true;
				unlisten?.();
			};
		},
		// Tauri's drag-region script already toggles maximise on double-click.
		handlesDoubleClickNatively: true,
	};
}

/** Shared instance for the common single-window case. */
export const tauriWindowControls: WindowControls = createTauriWindowControls();
