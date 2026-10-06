// The real NativeMenuClient: the app's `show_native_menu` command
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { NativeMenuClient } from './nativeMenuClient';

/** A `NativeMenuClient` over the `src-tauri` command. The command acts on the window that calls it. */
export function createTauriNativeMenuClient(): NativeMenuClient {
	return {
		show: (items, at) => invoke<string | null>('show_native_menu', { items, at }),
	};
}
