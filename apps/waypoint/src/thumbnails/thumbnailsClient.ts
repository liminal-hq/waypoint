// What the views need from the thumbnails plugin: its status, and queued, reprioritised and cancelled batches
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PluginStatus, ThumbEvent, ThumbSize, Ticket } from '@liminal-hq/plugin-thumbnails';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';

export type { PluginStatus, ThumbEvent, ThumbSize, Ticket };

/** An entry of an open listing, by the page's own key for it. */
export interface EntryRequest {
	key: string;
	id: EntryId;
}

/** A location (a Shelf item), by the page's own key for it. */
export interface LocationRequest {
	key: string;
	location: Location;
}

/**
 * The seam between the views and the plugin, injected so they are tested without one. The page
 * never sends a path: an entry is named by its listing handle and id, which `src-tauri` resolves
 * (A61), and keys are the page's own, which events come back under.
 */
export interface ThumbnailsClient {
	/** What works on this system, with the reason for what does not. */
	getStatus(): Promise<PluginStatus>;
	/** Queues thumbnails for entries of a listing the window has open. Each result arrives through `onEvent`. */
	requestEntries(
		handle: ListingHandle,
		items: EntryRequest[],
		size: ThumbSize,
		onEvent: (event: ThumbEvent) => void,
	): Promise<Ticket>;
	/** Queues thumbnails for locations (local files only; the plugin skips the rest). */
	requestLocations(
		items: LocationRequest[],
		size: ThumbSize,
		onEvent: (event: ThumbEvent) => void,
	): Promise<Ticket>;
	/** Moves the batch's pending items for `keys` to the front, first one first. */
	prioritise(ticket: Ticket, keys: string[]): Promise<void>;
	/** Withdraws a batch: nothing more is sent to it. */
	cancel(ticket: Ticket): Promise<boolean>;
}
