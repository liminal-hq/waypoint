// The real OsPaletteClient: the system-appearance plugin's palette through its guest-js API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getPalette, onPaletteChanged } from '@liminal-hq/plugin-system-appearance';
import type { OsPaletteClient } from './osPaletteClient';

/** An `OsPaletteClient` over the plugin. Outside Tauri every call fails quietly, and the page keeps its own colours. */
export function createTauriOsPaletteClient(): OsPaletteClient {
	return {
		get: async () => getPalette(),
		onChanged(listener) {
			let unlisten: (() => void) | undefined;
			let stopped = false;
			void (async () => onPaletteChanged(listener))().then(
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
