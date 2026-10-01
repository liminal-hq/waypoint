// The label of the webview this code runs in, which a job started here carries as its origin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

/** The current window's label, or `main` outside Tauri (a test, the browser demo). */
export function currentWindowLabel(): string {
	try {
		return getCurrentWebviewWindow().label;
	} catch {
		return 'main';
	}
}
