// Asking for a Properties window: which location the request is about, and what to tell the person when it is refused
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { selectedCount, type Selection } from '../browse/selection';
import type { ListingSession } from '../browse/useListingSession';
import { t, tf } from '../i18n/messages';
import type { PropertiesWindowClient } from '../services/propertiesWindowClient';
import type { VfsClient } from '../services/vfsClient';
import { postNotice } from '../tabs/notices';

/** The id of the only selected entry, when the selection names it. */
function onlyId(selection: Selection): number | null {
	if (selection.kind !== 'some' || selection.ids.size !== 1) return null;
	return selection.ids.values().next().value ?? null;
}

/**
 * What a request from `session` is about: `entry` (the right-clicked one) when the menu says so,
 * otherwise the one selected entry, or the folder itself when nothing is selected. `null` when
 * several are selected (a window is about one thing) or the entry cannot be named.
 */
export async function propertiesSubject(
	vfs: VfsClient,
	session: ListingSession,
	entry?: Entry,
): Promise<Location | null> {
	const { model, store } = session;
	if (entry) return vfs.entryLocation(model.handle, entry.id);
	const selection = store.getState().selection;
	const count = selectedCount(selection, model.count);
	if (count === 0) return model.location;
	if (count > 1) return null;
	const id = onlyId(selection);
	return id === null ? null : vfs.entryLocation(model.handle, id);
}

/** Opens (or focuses) the window for `location` and says so in the status bar when it is refused or fails. */
export async function openPropertiesWindowFor(
	client: PropertiesWindowClient,
	location: Location,
): Promise<void> {
	try {
		const outcome = await client.open(location);
		if (outcome === 'limit') postNotice(t('properties.limit'));
	} catch (error) {
		console.warn('could not open a Properties window', error);
		const reason = error instanceof Error ? error.message : String(error);
		postNotice(tf('properties.openFailed', { reason }));
	}
}
