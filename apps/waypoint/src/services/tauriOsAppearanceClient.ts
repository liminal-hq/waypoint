// The real OsAppearanceClient: the system-appearance plugin through its guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getAppearance, onAppearanceChanged } from '@liminal-hq/plugin-system-appearance';
import type { OsAppearanceClient } from './osAppearanceClient';

/** An `OsAppearanceClient` over the plugin. Outside Tauri every call fails quietly, and the caller falls back. */
export function createTauriOsAppearanceClient(): OsAppearanceClient {
	return {
		get: async () => getAppearance(),
		onChanged(listener) {
			let unlisten: (() => void) | undefined;
			let stopped = false;
			void (async () => onAppearanceChanged(listener))().then(
				(fn) => {
					if (stopped) fn();
					else unlisten = fn;
				},
				() => undefined,
			);
			return () => {
				stopped = true;
				unlisten?.();
				unlisten = undefined;
			};
		},
	};
}
