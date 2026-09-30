// The count and size of the selection: the count at once, the size from Rust a moment later
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SelectionSpec } from '@liminal-hq/waypoint-protocol/generated/SelectionSpec';
import { useEffect, useState, useSyncExternalStore } from 'react';
import { emptySelection, selectedCount, type Selection } from '../browse/selection';
import type { ListingSession } from '../browse/useListingSession';
import type { VfsClient } from '../services/vfsClient';

const NO_SUBSCRIBE = () => () => {};

/** How long the selection has to hold still before Rust is asked for its size. */
export const SUMMARY_DELAY_MS = 120;

export interface SelectionSummaryState {
	/** Selected entries, worked out here so it never lags a keypress. */
	count: number;
	/** Entries in the view. */
	total: number;
	/** The selected files' total size once Rust has it; the last known one while a newer is on its way. */
	size: number | null;
	/** Whether `size` is for an older selection. */
	pending: boolean;
	/** Whether the person has acted on the selection yet, so a fresh list announces nothing. */
	touched: boolean;
}

/** The wire form of a selection: the same shape, with the ids as an array. */
export function toSelectionSpec(selection: Selection): SelectionSpec {
	return selection.kind === 'some'
		? { kind: 'some', ids: [...selection.ids] }
		: { kind: 'allExcept', ids: [...selection.ids] };
}

/**
 * A selection can be "everything except these ids" over half a million rows, so its size is
 * summed in Rust (`VfsClient.summariseSelection`), debounced so holding an arrow key asks once.
 * The count comes from the selection model directly and is always current.
 */
export function useSelectionSummary(
	client: VfsClient,
	session: ListingSession | null,
): SelectionSummaryState {
	const store = session?.store;
	const model = session?.model;
	const subscribeStore = store ? store.subscribe : NO_SUBSCRIBE;
	const subscribeModel = model ? model.subscribe : NO_SUBSCRIBE;
	const selection = useSyncExternalStore(
		subscribeStore,
		() => store?.getState().selection ?? emptySelection,
	);
	const touched = useSyncExternalStore(subscribeStore, () => store?.getState().touched ?? false);
	const total = useSyncExternalStore(subscribeModel, () => model?.count ?? 0);
	const revision = useSyncExternalStore(subscribeModel, () => model?.revision ?? 0);

	const count = session ? selectedCount(selection, total) : 0;
	const nothing = count === 0;
	const [result, setResult] = useState<{
		selection: Selection;
		revision: number;
		size: number;
	} | null>(null);

	useEffect(() => {
		if (!model || nothing) {
			setResult(null);
			return;
		}
		let cancelled = false;
		const timer = setTimeout(() => {
			client.summariseSelection(model.handle, toSelectionSpec(selection)).then(
				(summary) => {
					if (!cancelled) setResult({ selection, revision, size: summary.totalSize });
				},
				() => {},
			);
		}, SUMMARY_DELAY_MS);
		return () => {
			cancelled = true;
			clearTimeout(timer);
		};
	}, [client, model, selection, revision, nothing]);

	return {
		count,
		total,
		size: nothing ? null : (result?.size ?? null),
		pending: result === null || result.selection !== selection || result.revision !== revision,
		touched,
	};
}
