// A folder's total size, counted in the background and cancelled the moment it is not wanted
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { FolderSizeTotals } from '@liminal-hq/waypoint-protocol/generated/FolderSizeTotals';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { useEffect, useState } from 'react';
import type { DetailsClient, FolderSizeJob } from '../services/detailsClient';
import { HEAVY_DELAY_MS } from './inspectorModel';

export type FolderSizeState =
	| { status: 'idle' }
	/** Counting; `totals` is the running count, `null` before the first progress. */
	| { status: 'calculating'; totals: FolderSizeTotals | null }
	| { status: 'done'; totals: FolderSizeTotals }
	| { status: 'failed' };

/**
 * Totals a folder through the cancellable `folderSize` stream. It starts only after the subject
 * has held still for `HEAVY_DELAY_MS`, and the run is cancelled when the subject changes, when
 * `active` goes false (the panel is closed or on another tab) and when the component goes away; a
 * run that is still starting when that happens is cancelled as soon as it has a job to cancel.
 */
export function useFolderSize(
	client: DetailsClient | null,
	handle: ListingHandle | null,
	id: EntryId | null,
	active: boolean,
): FolderSizeState {
	const [state, setState] = useState<{ key: string; value: FolderSizeState } | null>(null);
	const key = handle !== null && id !== null ? `${handle}:${id}` : null;
	useEffect(() => {
		if (!client || !active || handle === null || id === null || key === null) return;
		let live = true;
		let job: FolderSizeJob | null = null;
		const timer = setTimeout(() => {
			setState({ key, value: { status: 'calculating', totals: null } });
			client
				.folderSize(handle, id, (event) => {
					if (!live) return;
					if (event.kind === 'progress') {
						setState({ key, value: { status: 'calculating', totals: event.totals } });
					} else if (event.kind === 'done') {
						setState({ key, value: { status: 'done', totals: event.totals } });
					} else if (event.kind === 'failed') {
						setState({ key, value: { status: 'failed' } });
					}
				})
				.then(
					(started) => {
						job = started;
						// Cancelled while it was starting: stop it now there is a job to stop.
						if (!live) void started.cancel().catch(() => {});
					},
					(error: unknown) => {
						console.warn('could not total the folder', error);
						if (live) setState({ key, value: { status: 'failed' } });
					},
				);
		}, HEAVY_DELAY_MS);
		return () => {
			live = false;
			clearTimeout(timer);
			if (job) void job.cancel().catch(() => {});
		};
	}, [client, handle, id, key, active]);
	if (key === null || !active || state?.key !== key) {
		return client && active && key !== null
			? { status: 'calculating', totals: null }
			: { status: 'idle' };
	}
	return state.value;
}
