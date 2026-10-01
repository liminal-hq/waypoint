// Exposes typed guest-side wrappers for the waypoint-vfs plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { Filter } from '@liminal-hq/waypoint-protocol/generated/Filter';
import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { ListingSnapshot } from '@liminal-hq/waypoint-protocol/generated/ListingSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { LocationInfo } from '@liminal-hq/waypoint-protocol/generated/LocationInfo';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import type { Places } from '@liminal-hq/waypoint-protocol/generated/Places';
import type { SelectionSpec } from '@liminal-hq/waypoint-protocol/generated/SelectionSpec';
import type { SelectionSummary } from '@liminal-hq/waypoint-protocol/generated/SelectionSummary';
import type { TrashInfo } from '@liminal-hq/waypoint-protocol/generated/TrashInfo';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';

import type { FolderCheck } from '@liminal-hq/waypoint-protocol/generated/FolderCheck';
import type { VolumeSpace } from '@liminal-hq/waypoint-protocol/generated/VolumeSpace';

const PREFIX = 'plugin:waypoint-vfs|';
const LISTING_EVENT = 'waypoint-vfs://listing';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** How a listing opens; both parts default to name order with folders first, hidden files hidden. */
export interface OpenOptions {
	sort?: SortSpec;
	filter?: Filter;
}

/**
 * Reports whether the file system plugin works here, and which features: `listing`, `watch`,
 * `places`, `trash-view` when the Trash can be browsed, and `polling-fallback` while a listing is
 * kept up to date by polling.
 */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/**
 * Opens a listing of a folder and resolves with its first snapshot (phase `scanning`). The scan
 * continues in Rust and is reported through `onListingEvent`. Rejects with a `VfsError`.
 */
export function openListing(
	location: Location,
	options: OpenOptions = {},
): Promise<ListingSnapshot> {
	return cmd<ListingSnapshot>('open_listing', { location, options });
}

/** Reads `count` entries from view position `start`; shorter at the end of the listing. */
export function getRange(handle: ListingHandle, start: number, count: number): Promise<Entry[]> {
	return cmd<Entry[]>('get_range', { handle, start, count });
}

/** Re-sorts a listing and resolves with its new snapshot. Cached pages are stale afterwards. */
export function setSort(handle: ListingHandle, sort: SortSpec): Promise<ListingSnapshot> {
	return cmd<ListingSnapshot>('set_sort', { handle, sort });
}

/** Changes what a listing hides and resolves with its new snapshot. Cached pages are stale. */
export function setFilter(handle: ListingHandle, filter: Filter): Promise<ListingSnapshot> {
	return cmd<ListingSnapshot>('set_filter', { handle, filter });
}

/** Closes a listing, cancelling a scan in flight. Closing an unknown handle is not an error. */
export function closeListing(handle: ListingHandle): Promise<void> {
	return cmd<void>('close_listing', { handle });
}

/** The folder a window opens at first. */
export function getHome(): Promise<Location> {
	return cmd<Location>('get_home');
}

/**
 * Turns text the person typed (an absolute or relative path, `~`, a `file://` URI) into a
 * `Location`, resolving relative text against `base`. Rejects with `invalidLocation` or
 * `unsupported` (another scheme); it does not check that the location exists.
 */
export function parseLocation(input: string, base: Location): Promise<Location> {
	return cmd<Location>('parse_location', { input, base });
}

/** The parent and the breadcrumb segments of a location. */
export function describeLocation(location: Location): Promise<LocationInfo> {
	return cmd<LocationInfo>('describe_location', { location });
}

/** Where an entry of an open listing lives; `.display` is what Copy Path uses. */
export function entryLocation(handle: ListingHandle, id: EntryId): Promise<Location> {
	return cmd<Location>('entry_location', { handle, id });
}

/** The count and total file size of a selection over a listing's current view. */
export function summariseSelection(
	handle: ListingHandle,
	selection: SelectionSpec,
): Promise<SelectionSummary> {
	return cmd<SelectionSummary>('summarise_selection', { handle, selection });
}

/** Free and total space on the volume holding `location`, or `null` where it cannot be known. */
export function getFreeSpace(location: Location): Promise<VolumeSpace | null> {
	return cmd<VolumeSpace | null>('get_free_space', { location });
}

/**
 * Whether `location` is a folder and can be written to, for a destination picker. Rejects with
 * `notFound` (or `permissionDenied`) where it cannot be seen at all.
 */
export function checkFolder(location: Location): Promise<FolderCheck> {
	return cmd<FolderCheck>('check_folder', { location });
}

/**
 * Opens a file of an open listing in its default application. Rust resolves the path from
 * `(handle, id)`; a folder is rejected with `unsupported`.
 */
export function openEntry(handle: ListingHandle, id: EntryId): Promise<void> {
	return cmd<void>('open_entry', { handle, id });
}

/**
 * Whether the Trash can be browsed here, why not, and how many items it holds. Reading it lists the
 * Trash, so ask when the number is wanted (the sidebar does, on a slow timer and on focus).
 */
export function getTrashInfo(): Promise<TrashInfo> {
	return cmd<TrashInfo>('get_trash_info');
}

/** Home, the user folders that exist, and the favourites. */
export function listPlaces(): Promise<Places> {
	return cmd<Places>('list_places');
}

/** Pins a folder to the favourites, with an optional label. Resolves with the updated places. */
export function addFavourite(location: Location, label?: string): Promise<Places> {
	return cmd<Places>('add_favourite', { location, label: label ?? null });
}

/** Unpins a folder. Resolves with the updated places. */
export function removeFavourite(location: Location): Promise<Places> {
	return cmd<Places>('remove_favourite', { location });
}

/** Labels a favourite, or clears its label with `null`. Resolves with the updated places. */
export function renameFavourite(location: Location, label: string | null): Promise<Places> {
	return cmd<Places>('rename_favourite', { location, label });
}

/** Moves a favourite to position `to` in the list. Resolves with the updated places. */
export function moveFavourite(location: Location, to: number): Promise<Places> {
	return cmd<Places>('move_favourite', { location, to });
}

/**
 * Listens for progress, live patches and failures of every listing the calling window opened.
 * Subscribe first and then open listings, so no event between the two is missed. Resolves once
 * the listener is registered; call the returned function to stop listening.
 */
export function onListingEvent(handler: (_event: ListingEvent) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<ListingEvent>(LISTING_EVENT, (event) =>
		handler(event.payload),
	);
}

export type {
	Entry,
	EntryId,
	Filter,
	FolderCheck,
	ListingEvent,
	ListingHandle,
	ListingSnapshot,
	Location,
	LocationInfo,
	Places,
	PluginStatus,
	SelectionSpec,
	SelectionSummary,
	SortSpec,
	TrashInfo,
	VolumeSpace,
};
