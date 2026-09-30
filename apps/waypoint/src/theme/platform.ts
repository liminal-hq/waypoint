// Tags the root element with the host platform so CSS can adapt the window frame
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { platform, version } from '@tauri-apps/plugin-os';

/**
 * True for Windows 11. It reports the same `10.0.x` version string as Windows 10 and is
 * told apart only by build number: 22000 and up is Windows 11.
 */
export function isWindows11(osVersion: string): boolean {
	const build = Number(osVersion.split('.')[2]);
	return Number.isFinite(build) && build >= 22000;
}

/**
 * Sets `data-platform` on the root element to the `@tauri-apps/plugin-os` platform name
 * (`linux`, `windows`, `macos`, ...). Outside a Tauri webview (plain `bun run dev`,
 * tests) the lookup throws and the attribute is left unset, which the tokens treat as
 * "the OS draws the window shadow". On Windows it also sets `data-windows11`, because only
 * Windows 11 rounds frameless windows natively.
 */
export function applyPlatform(root: HTMLElement = document.documentElement): void {
	try {
		const current = platform();
		root.dataset.platform = current;
		if (current === 'windows') root.dataset.windows11 = String(isWindows11(version()));
	} catch {
		// Not running inside Tauri.
	}
}
