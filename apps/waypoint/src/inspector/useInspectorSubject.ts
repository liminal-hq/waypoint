// What the Inspector is about: the one selected entry, a count of several, or the folder itself
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useEffect, useState, useSyncExternalStore } from 'react';
import { emptySelection, selectedCount, type Selection } from '../browse/selection';
import type { ListingSession } from '../browse/useListingSession';
import { selectedEntries } from '../dnd/openFolders';

export type InspectorSubject =
	/** No listing is open. */
	| { kind: 'none' }
	/** Nothing is selected: the folder being shown. */
	| { kind: 'folder'; location: Location; count: number }
	| { kind: 'entry'; entry: Entry; handle: ListingHandle }
	| { kind: 'many'; count: number };

const NO_SUBSCRIBE = () => () => {};

/** The id of the only selected entry, when the selection names it. */
function onlyId(selection: Selection): EntryId | null {
	if (selection.kind !== 'some' || selection.ids.size !== 1) return null;
	return selection.ids.values().next().value ?? null;
}

/**
 * Follows the pane's selection. The count and the kind of subject are worked out from the
 * selection model at once; the one entry comes from the page of the listing the view already
 * holds (the keyboard's position first, which is where a click or an arrow key just put it), and
 * only when that has been evicted is it looked for, a page at a time. A listing change that
 * renames or replaces the entry gives the new one.
 */
export function useInspectorSubject(
	session: ListingSession | null,
	location: Location | undefined,
): InspectorSubject {
	const store = session?.store;
	const model = session?.model;
	const subscribeStore = store ? store.subscribe : NO_SUBSCRIBE;
	const subscribeModel = model ? model.subscribe : NO_SUBSCRIBE;
	const selection = useSyncExternalStore(
		subscribeStore,
		() => store?.getState().selection ?? emptySelection,
	);
	const focus = useSyncExternalStore(subscribeStore, () => store?.getState().focus ?? null);
	const total = useSyncExternalStore(subscribeModel, () => model?.count ?? 0);
	const revision = useSyncExternalStore(subscribeModel, () => model?.revision ?? 0);
	const count = session ? selectedCount(selection, total) : 0;

	const id = onlyId(selection);
	let near: Entry | undefined;
	if (model && count === 1) {
		const candidate = focus === null ? undefined : model.entryAt(focus);
		near = candidate && (id === null || candidate.id === id) ? candidate : undefined;
	}

	const [found, setFound] = useState<{
		selection: Selection;
		revision: number;
		entry: Entry | null;
	}>({ selection: emptySelection, revision: -1, entry: null });
	const needsSearch = count === 1 && near === undefined;
	useEffect(() => {
		if (!needsSearch || !model) return;
		let live = true;
		selectedEntries(model, selection, 1).then(
			(entries) => {
				if (live) setFound({ selection, revision, entry: entries[0] ?? null });
			},
			() => {},
		);
		return () => {
			live = false;
		};
	}, [needsSearch, model, selection, revision]);

	if (!session || !model) return { kind: 'none' };
	if (count === 0) return location ? { kind: 'folder', location, count: total } : { kind: 'none' };
	if (count > 1) return { kind: 'many', count };
	const entry =
		near ??
		(found.selection === selection && found.revision === revision ? found.entry : null) ??
		undefined;
	// Between the selection changing and the entry being found there is nothing to say yet.
	return entry ? { kind: 'entry', entry, handle: model.handle } : { kind: 'none' };
}
