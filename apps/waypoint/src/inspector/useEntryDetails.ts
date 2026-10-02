// The details of one entry, read a moment after the selection settles and dropped when it moves on
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryDetails } from '@liminal-hq/waypoint-protocol/generated/EntryDetails';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { useEffect, useState } from 'react';
import type { DetailsClient } from '../services/detailsClient';
import { DETAILS_DELAY_MS } from './inspectorModel';

export type DetailsState =
	{ status: 'loading' } | { status: 'ready'; details: EntryDetails } | { status: 'failed' };

/**
 * Reads `entryDetails` for the entry, after `DETAILS_DELAY_MS` of the selection holding still. A
 * reply for an entry that is no longer the subject is ignored, and the state is `loading` from the
 * moment the subject changes, so the panel never shows one file's facts under another's name. The
 * listing's own `modifiedMs` and `size` are dependencies: a file that changed is read again.
 */
export function useEntryDetails(
	client: DetailsClient | null,
	handle: ListingHandle | null,
	entry: Entry | null,
): DetailsState {
	const [result, setResult] = useState<{ key: string; state: DetailsState } | null>(null);
	const id = entry?.id ?? null;
	const key =
		handle !== null && id !== null ? `${handle}:${id}:${entry?.modifiedMs}:${entry?.size}` : null;
	useEffect(() => {
		if (!client || handle === null || id === null || key === null) return;
		let live = true;
		const timer = setTimeout(() => {
			client.entryDetails(handle, id).then(
				(details) => {
					if (live) setResult({ key, state: { status: 'ready', details } });
				},
				(error: unknown) => {
					console.warn('could not read the entry details', error);
					if (live) setResult({ key, state: { status: 'failed' } });
				},
			);
		}, DETAILS_DELAY_MS);
		return () => {
			live = false;
			clearTimeout(timer);
		};
	}, [client, handle, id, key]);
	if (key === null || result?.key !== key) return { status: 'loading' };
	return result.state;
}
