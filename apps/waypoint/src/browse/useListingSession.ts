// Opens a listing for a location and hands the view its model and per-listing store
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { useEffect, useState } from 'react';
import type { VfsClient } from '../services/vfsClient';
import { createBrowseStore, type BrowseStore } from './browseStore';
import { openListingModel, toVfsError, type ListingModel } from './listingModel';

/** What a view keeps about how it was left, so a tab that returns finds its scroll position. */
export interface ViewMemory {
	scrollTop: number;
	/** The grid's own offset: a pixel offset means something different in each layout. */
	gridScrollTop: number;
	/**
	 * A restored offset the view has yet to reach: the listing may still be growing, so the view
	 * keeps trying as it does and ignores scroll events meanwhile. `null` once applied.
	 */
	pendingScroll: number | null;
}

export interface ListingSession {
	model: ListingModel;
	store: BrowseStore;
	view: ViewMemory;
}

export function createListingSession(model: ListingModel): ListingSession {
	return {
		model,
		store: createBrowseStore(model),
		view: { scrollTop: 0, gridScrollTop: 0, pendingScroll: null },
	};
}

export type SessionState =
	| { status: 'opening' }
	| { status: 'ready'; session: ListingSession }
	| { status: 'error'; error: VfsError };

/**
 * Opens the listing of `location` and closes it again when the location changes or the view goes
 * away. The selection store is created with the model, so it is scoped to this listing and starts
 * empty for the next one.
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
				setState({ status: 'ready', session: createListingSession(model) });
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
