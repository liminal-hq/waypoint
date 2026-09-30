// Opens a listing for a location and hands the view its model and per-listing store
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { useEffect, useState } from 'react';
import type { VfsClient } from '../services/vfsClient';
import { openListingModel, toVfsError, type ListingModel } from './listingModel';

export interface ListingSession {
	model: ListingModel;
}

export type SessionState =
	| { status: 'opening' }
	| { status: 'ready'; session: ListingSession }
	| { status: 'error'; error: VfsError };

/**
 * Opens the listing of `location` and closes it again when the location changes or the view goes
 * away.
 */
export function useListingSession(client: VfsClient, location: Location): SessionState {
	const [state, setState] = useState<SessionState>({ status: 'opening' });
	const uri = location.uri;

	useEffect(() => {
		let cancelled = false;
		let opened: ListingModel | null = null;
		setState({ status: 'opening' });
		openListingModel(client, location).then(
			(model) => {
				if (cancelled) {
					model.dispose();
					return;
				}
				opened = model;
				setState({ status: 'ready', session: { model } });
			},
			(error: unknown) => {
				if (!cancelled) setState({ status: 'error', error: toVfsError(error) });
			},
		);
		return () => {
			cancelled = true;
			opened?.dispose();
		};
		// `location` is identified by its uri: a new object for the same folder must not reopen it.
	}, [client, uri]);

	return state;
}
