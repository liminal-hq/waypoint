// Per-listing UI state: the selection, the keyboard focus and the range anchor, following the listing's patches
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import { createStore, type StoreApi } from 'zustand/vanilla';
import type { ListingModel, PatchReport } from './listingModel';
import { isReset, mapPosition } from './patch';
import {
	addIds,
	emptySelection,
	everything,
	invert,
	normalise,
	rangeBetween,
	removeIds,
	selectIds,
	selectOnly,
	toggle,
	type Selection,
} from './selection';

export interface BrowseState {
	selection: Selection;
	/** The view position a Shift range starts from. */
	anchor: number | null;
	/** The view position the keyboard is on (`aria-activedescendant`), selected or not. */
	focus: number | null;
	/** Whether the person has acted on the selection yet, so a fresh list announces nothing. */
	touched: boolean;
}

export interface BrowseActions {
	/** Plain click: replaces the selection with one entry. */
	click(position: number, id: EntryId): void;
	/** Ctrl-click: flips one entry. */
	toggleAt(position: number, id: EntryId): void;
	/** Shift-click and Shift+arrows: the range from the anchor, replacing or (`additive`) adding. */
	extendTo(position: number, additive?: boolean): Promise<void>;
	/** Moves the keyboard focus, selecting the entry there unless `select` is false. Returns the clamped position. */
	moveTo(position: number, select: boolean): number | null;
	/** Ctrl+Space: flips the focused entry. */
	toggleFocused(): void;
	selectAll(): void;
	invertSelection(): void;
	deselectAll(): void;
}

export type BrowseStore = StoreApi<BrowseState & BrowseActions>;

/**
 * Creates the store for one listing. Selection is keyed by entry id, so a re-sort or a patch leaves
 * it alone except to forget ids that were removed; the anchor and focus are view positions and are
 * carried through each patch with `mapPosition`.
 */
export function createBrowseStore(model: ListingModel): BrowseStore {
	// Bumped by every change to the selection, so an older asynchronous range read that finishes
	// after a newer action cannot overwrite it.
	let epoch = 0;

	const store: BrowseStore = createStore<BrowseState & BrowseActions>()((set, get) => {
		const commit = (selection: Selection) =>
			set({ selection: normalise(selection, model.count), touched: true });

		return {
			selection: emptySelection,
			anchor: null,
			focus: null,
			touched: false,

			click(position, id) {
				epoch++;
				set({ anchor: position, focus: position });
				commit(selectOnly(id));
			},

			toggleAt(position, id) {
				epoch++;
				set({ anchor: position, focus: position });
				commit(toggle(get().selection, id));
			},

			async extendTo(position, additive = false) {
				const target = clamp(position, model.count);
				if (target === null) return;
				const anchor = get().anchor ?? get().focus ?? target;
				const mine = ++epoch;
				set({ anchor, focus: target });
				const [start, end] = rangeBetween(anchor, target);
				if (start === 0 && end >= model.count && !additive) {
					commit(everything);
					return;
				}
				const ids = (await model.readRange(start, end)).map((entry) => entry.id);
				if (mine !== epoch) return;
				commit(additive ? addIds(get().selection, ids) : selectIds(ids));
			},

			moveTo(position, select) {
				const target = clamp(position, model.count);
				if (target === null) return null;
				set({ focus: target });
				if (!select) return target;
				const mine = ++epoch;
				set({ anchor: target });
				const cached = model.hasFresh(target) ? model.entryAt(target) : undefined;
				if (cached) {
					commit(selectOnly(cached.id));
				} else {
					void model.idAt(target).then((id) => {
						if (mine === epoch && id !== undefined) commit(selectOnly(id));
					});
				}
				return target;
			},

			toggleFocused() {
				const { focus } = get();
				if (focus === null) return;
				const mine = ++epoch;
				set({ anchor: focus });
				void model.idAt(focus).then((id) => {
					if (mine === epoch && id !== undefined) commit(toggle(get().selection, id));
				});
			},

			selectAll() {
				epoch++;
				commit(everything);
			},

			invertSelection() {
				epoch++;
				commit(invert(get().selection));
			},

			deselectAll() {
				epoch++;
				commit(emptySelection);
			},
		};
	});

	const follow = (report: PatchReport) => {
		epoch++;
		const { selection, anchor, focus } = store.getState();
		const reset = isReset(report.ops);
		const move = (position: number | null): number | null => {
			if (position === null) return null;
			const next = reset ? position : mapPosition(position, report.ops).position;
			return clamp(next, report.count);
		};
		store.setState({
			selection: normalise(removeIds(selection, report.removedIds), report.count),
			anchor: move(anchor),
			focus: move(focus),
		});
	};
	model.onPatch(follow);

	// A scan growing or shrinking the listing can leave the focus past its end.
	model.subscribe(() => {
		const { focus, anchor } = store.getState();
		const clampedFocus = focus === null ? null : clamp(focus, model.count);
		const clampedAnchor = anchor === null ? null : clamp(anchor, model.count);
		if (clampedFocus !== focus || clampedAnchor !== anchor) {
			store.setState({ focus: clampedFocus, anchor: clampedAnchor });
		}
	});

	return store;
}

function clamp(position: number, count: number): number | null {
	if (count === 0) return null;
	return Math.max(0, Math.min(count - 1, position));
}
