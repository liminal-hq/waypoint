// The OS colour palette as the theme uses it: read once, and hear every change
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Palette, PaletteEntry } from '@liminal-hq/plugin-system-appearance';
import type { Unsubscribe } from './vfsClient';

export type { Palette, PaletteEntry };

export interface OsPaletteClient {
	/** Reads the palette now; rejects where the plugin is not there or the call fails. */
	get(): Promise<Palette>;
	/** Calls `listener` with each change (every one carries a rising revision). */
	onChanged(listener: (palette: Palette) => void): Unsubscribe;
}
