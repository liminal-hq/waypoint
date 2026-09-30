// Tags the root element with the host platform so CSS can adapt the window frame
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { platform } from '@tauri-apps/plugin-os';

/**
 * Sets `data-platform` on the root element to the `@tauri-apps/plugin-os` platform name
 * (`linux`, `windows`, `macos`, ...). Outside a Tauri webview (plain `bun run dev`,
 * tests) the lookup throws and the attribute is left unset, which the tokens treat as
 * "the OS draws the window shadow".
 */
export function applyPlatform(root: HTMLElement = document.documentElement): void {
	try {
		root.dataset.platform = platform();
	} catch {
		// Not running inside Tauri.
	}
}
