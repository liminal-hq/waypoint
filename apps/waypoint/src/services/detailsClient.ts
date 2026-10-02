// The frontend's view of entry details: the Inspector's facts, a folder's total size and a text head
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EntryDetails } from '@liminal-hq/waypoint-protocol/generated/EntryDetails';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { FolderSizeEvent } from '@liminal-hq/waypoint-protocol/generated/FolderSizeEvent';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { TextHead } from '@liminal-hq/waypoint-protocol/generated/TextHead';

/** A running folder-size total. */
export interface FolderSizeJob {
	/** The run's id. */
	readonly job: number;
	/** Stops the run; it ends with a `cancelled` event carrying the partial total. */
	cancel(): Promise<void>;
}

/**
 * What the Inspector, Properties and Quick Look need to know about an entry of a listing this
 * window opened. Everything is addressed by `(handle, id)`: Rust resolves the path, and another
 * window's handle is `staleHandle`.
 *
 * Every method rejects with a `VfsError` when something goes wrong. `TauriDetailsClient` wraps the
 * `waypoint-vfs` plugin; `FakeDetailsClient` serves the same contract from memory.
 */
export interface DetailsClient {
	/**
	 * Kind, exact and allocated size, times, owner and group, permissions, symlink target, hidden
	 * flag and content type. A field the provider cannot report is named in `unavailable`; a
	 * field the entry does not have is `null`.
	 */
	entryDetails(handle: ListingHandle, id: EntryId): Promise<EntryDetails>;
	/**
	 * Starts totalling a folder and resolves once it has started. `onEvent` gets `progress` and then
	 * exactly one `done`, `cancelled` or `failed`. Rejects (`notADirectory`) for a non-folder.
	 */
	folderSize(
		handle: ListingHandle,
		id: EntryId,
		onEvent: (event: FolderSizeEvent) => void,
	): Promise<FolderSizeJob>;
	/**
	 * The first bytes of a file as text (at most `max`, and never more than 256 KiB). Rejects with
	 * `notText` for a binary file and `isADirectory` for a folder.
	 */
	readTextHead(handle: ListingHandle, id: EntryId, max?: number): Promise<TextHead>;
	/**
	 * The URL that serves the entry's bytes (with `Range`) to an `<img>`, `<audio>`, `<video>` or
	 * `fetch`. It is a token over this window's own listings, never a path.
	 */
	previewUrl(handle: ListingHandle, id: EntryId): string;
}
