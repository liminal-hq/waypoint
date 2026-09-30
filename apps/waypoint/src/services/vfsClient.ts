// The frontend's view of the file system plugin: open a listing, read ranges of it, follow changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { Filter } from '@liminal-hq/waypoint-protocol/generated/Filter';
import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { ListingSnapshot } from '@liminal-hq/waypoint-protocol/generated/ListingSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { LocationInfo } from '@liminal-hq/waypoint-protocol/generated/LocationInfo';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';

export type Unsubscribe = () => void;

export interface OpenOptions {
	sort?: SortSpec;
	filter?: Filter;
}

/**
 * Everything the file views need from the file system plugin. Rust owns the listing and the
 * frontend holds only the pages near the viewport (A9), so nothing here returns a whole folder.
 *
 * Every method rejects with a `VfsError` when something goes wrong. A real implementation wraps the
 * `waypoint-vfs` plugin's guest-js API; `FakeVfsClient` serves the same contract from memory so
 * the views can be built and tested before (and independently of) the Rust side.
 */
export interface VfsClient {
	/** Opens a listing of a folder and returns its state at the first revision. */
	openListing(location: Location, options?: OpenOptions): Promise<ListingSnapshot>;
	/** Reads `count` entries from view position `start`; shorter at the end of the listing. */
	getRange(handle: ListingHandle, start: number, count: number): Promise<Entry[]>;
	/** Re-sorts the listing and returns its new state. Cached pages are stale afterwards. */
	setSort(handle: ListingHandle, sort: SortSpec): Promise<ListingSnapshot>;
	/** Changes what the listing hides and returns its new state. Cached pages are stale. */
	setFilter(handle: ListingHandle, filter: Filter): Promise<ListingSnapshot>;
	/** Closes a listing and releases its memory. Closing an unknown handle is not an error. */
	closeListing(handle: ListingHandle): Promise<void>;
	/**
	 * Turns text the person typed (an absolute or relative path, `~`, a `file://` URI) into a
	 * `Location`, resolving relative text against `base`. Rejects with `invalidLocation` when the
	 * text names nothing parseable; it does not check that the location exists, so a missing folder
	 * is found (and shown as its own state) when the listing opens.
	 */
	parseLocation(input: string, base: Location): Promise<Location>;
	/** The parent and the breadcrumb segments of a location; Rust splits the path, never the UI. */
	describeLocation(location: Location): Promise<LocationInfo>;
	/** Where an entry of an open listing lives, so a folder can be opened and its path copied. */
	entryLocation(handle: ListingHandle, id: EntryId): Promise<Location>;
	/** Follows progress, live patches and failures for every listing this client opened. */
	onListingEvent(listener: (event: ListingEvent) => void): Unsubscribe;
}

/** Whether a rejected value is a `VfsError` from the file system plugin. */
export function isVfsError(value: unknown): value is VfsError {
	return (
		typeof value === 'object' &&
		value !== null &&
		typeof (value as { kind?: unknown }).kind === 'string'
	);
}
