// Exposes typed guest-side wrappers for the waypoint-vfs plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Channel, convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { DirScanEvent } from '@liminal-hq/waypoint-protocol/generated/DirScanEvent';
import type { DirScanOptions } from '@liminal-hq/waypoint-protocol/generated/DirScanOptions';
import type { DirScanResult } from '@liminal-hq/waypoint-protocol/generated/DirScanResult';
import type { EntryDetails } from '@liminal-hq/waypoint-protocol/generated/EntryDetails';
import type { FolderSizeEvent } from '@liminal-hq/waypoint-protocol/generated/FolderSizeEvent';
import type { FolderSizeTotals } from '@liminal-hq/waypoint-protocol/generated/FolderSizeTotals';
import type { TextHead } from '@liminal-hq/waypoint-protocol/generated/TextHead';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { Filter } from '@liminal-hq/waypoint-protocol/generated/Filter';
import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { ListingSnapshot } from '@liminal-hq/waypoint-protocol/generated/ListingSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { LocationInfo } from '@liminal-hq/waypoint-protocol/generated/LocationInfo';
import type { TypedLocation } from '@liminal-hq/waypoint-protocol/generated/TypedLocation';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import type { Places } from '@liminal-hq/waypoint-protocol/generated/Places';
import type { SelectionSpec } from '@liminal-hq/waypoint-protocol/generated/SelectionSpec';
import type { SelectionSummary } from '@liminal-hq/waypoint-protocol/generated/SelectionSummary';
import type { TrashInfo } from '@liminal-hq/waypoint-protocol/generated/TrashInfo';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';

import type { FolderCheck } from '@liminal-hq/waypoint-protocol/generated/FolderCheck';
import type { GroupBy } from '@liminal-hq/waypoint-protocol/generated/GroupBy';
import type { GroupKey } from '@liminal-hq/waypoint-protocol/generated/GroupKey';
import type { GroupRun } from '@liminal-hq/waypoint-protocol/generated/GroupRun';
import type { VolumeSpace } from '@liminal-hq/waypoint-protocol/generated/VolumeSpace';
import type { AnswerInput } from '@liminal-hq/waypoint-protocol/generated/AnswerInput';
import type { ConnectionDraft } from '@liminal-hq/waypoint-protocol/generated/ConnectionDraft';
import type { ConnectionEntry } from '@liminal-hq/waypoint-protocol/generated/ConnectionEntry';
import type { ConnectionStatus } from '@liminal-hq/waypoint-protocol/generated/ConnectionStatus';
import type { ConnectionSupport } from '@liminal-hq/waypoint-protocol/generated/ConnectionSupport';
import type { ConnectionsChanged } from '@liminal-hq/waypoint-protocol/generated/ConnectionsChanged';
import type { ConnectionsOverview } from '@liminal-hq/waypoint-protocol/generated/ConnectionsOverview';
import type { KeyringUnavailable } from '@liminal-hq/waypoint-protocol/generated/KeyringUnavailable';
import type { ProtocolsChanged } from '@liminal-hq/waypoint-protocol/generated/ProtocolsChanged';
import type { ParsedAddress } from '@liminal-hq/waypoint-protocol/generated/ParsedAddress';
import type { Remembered } from '@liminal-hq/waypoint-protocol/generated/Remembered';
import type { SuggestedServer } from '@liminal-hq/waypoint-protocol/generated/SuggestedServer';
import type { TestedConnection } from '@liminal-hq/waypoint-protocol/generated/TestedConnection';

const PREFIX = 'plugin:waypoint-vfs|';
const LISTING_EVENT = 'waypoint-vfs://listing';
const CONNECTIONS_EVENT = 'waypoint-vfs://connections';
const CONNECTION_STATE_EVENT = 'waypoint-vfs://connection-state';
const PROTOCOLS_EVENT = 'waypoint-vfs://protocols';

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
 * `places`, `entry-details`, `folder-size`, `text-head`, `preview-protocol`, `trash-view` when the Trash can be browsed, and `polling-fallback` while a listing is
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
/**
 * `parseLocation` for typed text in the path bar: also says whether a password written in a server
 * address was dropped (it is never kept, D147).
 */
export function parseLocationText(input: string, base: Location): Promise<TypedLocation> {
	return cmd<TypedLocation>('parse_location_text', { input, base });
}

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
 * Everything the Inspector shows about one entry of an open listing: kind, exact and allocated
 * size, times, owner and group, permissions, symlink target, hidden flag and content type. A field
 * the provider cannot report is named in `unavailable`; a field the entry does not have is `null`.
 */
export function entryDetails(handle: ListingHandle, id: EntryId): Promise<EntryDetails> {
	return cmd<EntryDetails>('entry_details', { handle, id });
}

/** A running folder-size total. */
export interface FolderSizeRun {
	/** The run's id, which `cancelFolderSize` takes. */
	job: number;
	/** Stops the run; it ends with a `cancelled` event carrying the partial total. */
	cancel(): Promise<void>;
}

/**
 * Starts totalling a folder of an open listing and resolves with the run as soon as it has
 * started. `onEvent` gets `progress` about every 100 ms and then exactly one `done`, `cancelled`
 * or `failed`. The walk is low priority, stays on one volume, never follows a symlink and never
 * downloads a cloud placeholder. Rejects (`notADirectory`) for an entry that is not a folder.
 */
export async function folderSize(
	handle: ListingHandle,
	id: EntryId,
	onEvent: (event: FolderSizeEvent) => void,
): Promise<FolderSizeRun> {
	const channel = new Channel<FolderSizeEvent>();
	channel.onmessage = onEvent;
	const job = await cmd<number>('folder_size', { handle, id, onEvent: channel });
	return { job, cancel: () => cancelFolderSize(job) };
}

/** Stops a folder-size run of this window. A run that has ended is not an error. */
export function cancelFolderSize(job: number): Promise<void> {
	return cmd<void>('cancel_folder_size', { job });
}

/** A running directory-size scan. */
export interface DirScanRun {
	/** The run's id, which `cancelDirScan` takes. */
	job: number;
	/** Stops the scan; it ends with a `cancelled` event carrying the folders finished so far. */
	cancel(): Promise<void>;
}

/**
 * Starts scanning the top-level folders of `location` for their sizes and resolves with the run as
 * soon as it has started. `onEvent` gets `progress` about every 100 ms, a `partial` result after
 * each top-level folder (every row so far, with its share of what has been scanned, and a
 * remainder row for loose files and hidden items), and then exactly one `done`, `cancelled` or
 * `failed`. The scan is low priority on a thread of its own, stays on one volume, never follows a
 * symlink and never downloads a cloud placeholder. A finished scan is cached for
 * `getCachedDirScan`.
 */
export async function scanDirSizes(
	location: Location,
	onEvent: (event: DirScanEvent) => void,
	options?: DirScanOptions,
): Promise<DirScanRun> {
	const channel = new Channel<DirScanEvent>();
	channel.onmessage = onEvent;
	const job = await cmd<number>('scan_dir_sizes', { location, options, onEvent: channel });
	return { job, cancel: () => cancelDirScan(job) };
}

/** Stops a directory-size scan of this window. A scan that has ended is not an error. */
export function cancelDirScan(job: number): Promise<void> {
	return cmd<void>('cancel_dir_scan', { job });
}

/**
 * The last finished scan of `location`, with `measuredAtMs` for "as of <time>", or `null` when
 * there is none.
 */
export function getCachedDirScan(location: Location): Promise<DirScanResult | null> {
	return cmd<DirScanResult | null>('get_cached_dir_scan', { location });
}

/**
 * The first bytes of a file of an open listing as text: at most `max` bytes (default and ceiling
 * 256 KiB), decoded as UTF-8 with invalid sequences replaced. Rejects with `notText` for a binary
 * file and `isADirectory` for a folder.
 */
export function readTextHead(handle: ListingHandle, id: EntryId, max?: number): Promise<TextHead> {
	return cmd<TextHead>('read_text_head', { handle, id, max: max ?? null });
}

/** The custom scheme that serves entries to this window. */
export const PREVIEW_SCHEME = 'wpfile';

/**
 * The URL that serves an entry's bytes through the `wpfile` protocol, for an `<img>`, `<audio>`,
 * `<video>` or `fetch`, with `Range` support. It is a token over this window's own listings, never
 * a path: another window's URL, a closed listing and an entry that has gone all answer 404.
 */
export function previewUrl(handle: ListingHandle, id: EntryId): string {
	return convertFileSrc(`${handle}-${id}`, PREVIEW_SCHEME);
}

/**
 * Whether the Trash can be browsed here, why not, and how many items it holds. Reading it lists the
 * Trash, so ask when the number is wanted (the sidebar does, on a slow timer and on focus).
 * `withBytes` also adds up the sizes into `totalBytes`; ask only while Overview is visible.
 */
export function getTrashInfo(withBytes = false): Promise<TrashInfo> {
	return cmd<TrashInfo>('get_trash_info', { withBytes });
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

/** The saved connections and recent servers, with the state of every login Rust knows. */
export function listConnections(): Promise<ConnectionsOverview> {
	return cmd<ConnectionsOverview>('list_connections');
}

/**
 * The server protocols a provider serves here, and why a login cannot be remembered in the keyring
 * (`null` when it can). Never asks the keyring to unlock.
 */
export function connectionSupport(): Promise<ConnectionSupport> {
	return cmd<ConnectionSupport>('connection_support');
}

/** Hosts of `~/.ssh/config` to offer in the Connect dialog. */
export function suggestedServers(): Promise<SuggestedServer[]> {
	return cmd<SuggestedServer[]>('suggested_servers');
}

/**
 * Reads a typed server address into the dialog's fields, saying whether a password written in it
 * was dropped. Rejects with a `VfsError` (`invalidLocation`, or `unsupported` for a protocol no
 * provider serves).
 */
export function parseAddress(text: string): Promise<ParsedAddress> {
	return cmd<ParsedAddress>('parse_address_text', { text });
}

/** Saves a new connection. Rejects with `{ kind: 'connections', error }` naming the bad field. */
export function addConnection(draft: ConnectionDraft): Promise<ConnectionEntry> {
	return cmd<ConnectionEntry>('add_connection', { draft });
}

/** Changes a saved connection. */
export function updateConnection(id: string, draft: ConnectionDraft): Promise<ConnectionEntry> {
	return cmd<ConnectionEntry>('update_connection', { id, draft });
}

/** Saves a copy of a connection right after it, under `name`. */
export function duplicateConnection(id: string, name: string): Promise<ConnectionEntry> {
	return cmd<ConnectionEntry>('duplicate_connection', { id, name });
}

/**
 * Forgets a saved connection; with `forgetLogin` its remembered secrets go too. Resolves with why
 * the keyring could not forget them, or `null`.
 */
export function removeConnection(
	id: string,
	forgetLogin: boolean,
): Promise<KeyringUnavailable | null> {
	return cmd<KeyringUnavailable | null>('remove_connection', { id, forgetLogin });
}

/** Moves a saved connection to position `to`. */
export function moveConnection(id: string, to: number): Promise<void> {
	return cmd<void>('move_connection', { id, to });
}

/** Forgets one recent server by its login, or all of them with `null`. */
export function forgetRecentServer(key: string | null): Promise<void> {
	return cmd<void>('forget_recent_server', { key });
}

/** Forgets the remembered secrets of a server's login. Resolves with why it could not, or `null`. */
export function forgetLogin(location: Location): Promise<KeyringUnavailable | null> {
	return cmd<KeyringUnavailable | null>('forget_login', { location });
}

/**
 * Connects a server's login now, with the person's answer to the question its last attempt asked
 * (none retries, as Reconnect does). The answer, which may hold a secret, is sent once and never
 * comes back. Rejects with the `VfsError` that says what is still needed.
 */
export function connect(
	location: Location,
	answer: AnswerInput | null = null,
	remember = false,
): Promise<Remembered> {
	return cmd<Remembered>('connect', { location, answer, remember });
}

/** Tries a draft's server without saving it, as `connect` does, and says where the draft opens. */
export function testConnection(
	draft: ConnectionDraft,
	answer: AnswerInput | null = null,
	remember = false,
): Promise<TestedConnection> {
	return cmd<TestedConnection>('test_connection', { draft, answer, remember });
}

/** Closes a server's login; its listings show the disconnected state. */
export function disconnect(location: Location): Promise<void> {
	return cmd<void>('disconnect', { location });
}

/** The state of the login a location belongs to, or `null` for one with no login. */
export function connectionState(location: Location): Promise<ConnectionStatus | null> {
	return cmd<ConnectionStatus | null>('connection_state', { location });
}

/** Hears every change to the saved connections and recent servers, in any window. */
export function onConnectionsChanged(
	handler: (_change: ConnectionsChanged) => void,
): Promise<UnlistenFn> {
	return listen<ConnectionsChanged>(CONNECTIONS_EVENT, (event) => handler(event.payload));
}

/** Hears every time a remote protocol is turned on or off in Settings → Experimental, in any window. */
export function onProtocolsChanged(
	handler: (_change: ProtocolsChanged) => void,
): Promise<UnlistenFn> {
	return listen<ProtocolsChanged>(PROTOCOLS_EVENT, (event) => handler(event.payload));
}

/** Hears every change of a login's state. */
export function onConnectionState(
	handler: (_status: ConnectionStatus) => void,
): Promise<UnlistenFn> {
	return listen<ConnectionStatus>(CONNECTION_STATE_EVENT, (event) => handler(event.payload));
}

export type {
	AnswerInput,
	ConnectionDraft,
	ConnectionEntry,
	ConnectionStatus,
	ConnectionSupport,
	ConnectionsChanged,
	ConnectionsOverview,
	KeyringUnavailable,
	ParsedAddress,
	ProtocolsChanged,
	Remembered,
	SuggestedServer,
	TestedConnection,
	DirScanEvent,
	DirScanOptions,
	DirScanResult,
	Entry,
	EntryDetails,
	FolderSizeEvent,
	FolderSizeTotals,
	TextHead,
	EntryId,
	Filter,
	FolderCheck,
	GroupBy,
	GroupKey,
	GroupRun,
	ListingEvent,
	ListingHandle,
	ListingSnapshot,
	Location,
	LocationInfo,
	TypedLocation,
	Places,
	PluginStatus,
	SelectionSpec,
	SelectionSummary,
	SortSpec,
	TrashInfo,
	VolumeSpace,
};
