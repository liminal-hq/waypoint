// What each folder remembers about its view, sort, grouping and column widths, as the window uses it: read, write and follow
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	FolderView,
	FolderViewPatch,
	FolderViewsChanged,
	FolderViewsSnapshot,
	ListColumnWidths,
} from '@liminal-hq/waypoint-plugin-settings';
import type { Unsubscribe } from './vfsClient';

export type {
	FolderView,
	FolderViewPatch,
	FolderViewsChanged,
	FolderViewsSnapshot,
	ListColumnWidths,
};

/** The widest and narrowest a list column may be given, in pixels, which Rust enforces; the list keeps each column to a narrower range of its own. */
export const COLUMN_WIDTH_MIN = 32;
export const COLUMN_WIDTH_MAX = 1200;

/**
 * Everything the browsing window asks of the remembered folder views. Rust owns them: a write
 * asks for a change, the answer is the revision in force, and every window (this one included)
 * hears the change as an event. `FakeFolderViewsClient` keeps the same contract in memory.
 */
export interface FolderViewsClient {
	/** Every remembered folder and the revision they are at. */
	snapshot(): Promise<FolderViewsSnapshot>;
	/**
	 * Remembers what was chosen for the folder at `key` (its location's `uri`), on top of what it
	 * already remembers: a field that is `null` is kept as it is. `columnWidths` replaces the
	 * folder's whole set, and a set with no width in it makes the folder forget its widths.
	 */
	remember(key: string, patch: FolderViewPatch): Promise<number>;
	/** Makes the folder forget its own view. */
	reset(key: string): Promise<number>;
	onChanged(listener: (changed: FolderViewsChanged) => void): Unsubscribe;
}
