// The locations Open With acts on: the right-clicked entry, or the whole selection it is part of
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { isSelected } from '../browse/selection';
import type { ListingSession } from '../browse/useListingSession';
import { selectedEntries } from '../dnd/openFolders';
import type { VfsClient } from '../services/vfsClient';

/** The most locations one Open With starts; a larger selection is not offered it, as it would start that many windows. */
export const OPEN_WITH_LIMIT = 50;

/** True for a local file or folder, which is what the applications on this computer open. */
export function isLocalUri(uri: string): boolean {
	return uri.startsWith('file://');
}

/**
 * The URIs of what Open With acts on: the selection when `entry` is part of it, else `entry`
 * alone. `null` when there are more than `OPEN_WITH_LIMIT`, or any is not on this computer.
 */
export async function openWithUris(
	vfs: Pick<VfsClient, 'entryLocation'>,
	session: ListingSession | null,
	entry: Entry,
	handle: ListingHandle,
): Promise<string[] | null> {
	let entries: Entry[] = [entry];
	if (session && isSelected(session.store.getState().selection, entry.id)) {
		entries = await selectedEntries(
			session.model,
			session.store.getState().selection,
			OPEN_WITH_LIMIT + 1,
		);
		if (entries.length > OPEN_WITH_LIMIT) return null;
	}
	const from = session?.model.handle ?? handle;
	const uris = (await Promise.all(entries.map((each) => vfs.entryLocation(from, each.id)))).map(
		(location) => location.uri,
	);
	return uris.length > 0 && uris.every(isLocalUri) ? uris : null;
}

/** The URIs of the active pane's selection, for the command; `null` under the same rules. */
export async function selectionUris(
	vfs: Pick<VfsClient, 'entryLocation'>,
	session: ListingSession | null,
): Promise<string[] | null> {
	if (!session) return null;
	const { selection } = session.store.getState();
	const entries = await selectedEntries(session.model, selection, OPEN_WITH_LIMIT + 1);
	if (entries.length === 0 || entries.length > OPEN_WITH_LIMIT) return null;
	const uris = (
		await Promise.all(entries.map((each) => vfs.entryLocation(session.model.handle, each.id)))
	).map((location) => location.uri);
	return uris.every(isLocalUri) ? uris : null;
}
