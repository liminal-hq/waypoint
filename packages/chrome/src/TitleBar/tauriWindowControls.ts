// Tauri implementation of the WindowControls adapter
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getCurrentWindow } from '@tauri-apps/api/window';
import { subscription } from './subscription';
import type { WindowControls } from './windowControls';

/** Creates an adapter bound to the current Tauri window. Resolves the window lazily on each call. */
export function createTauriWindowControls(): WindowControls {
	return {
		minimize: () => getCurrentWindow().minimize(),
		toggleMaximize: () => getCurrentWindow().toggleMaximize(),
		close: () => getCurrentWindow().close(),
		startDragging: () => getCurrentWindow().startDragging(),
		setTitle: (title) => getCurrentWindow().setTitle(title),
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
