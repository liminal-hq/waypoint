// The selection model: which entries of a listing are selected, keyed by id so it survives re-sorts and patches
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';

/**
 * Either "these ids" or "everything except these ids". The second form is what makes select all
 * and invert O(1) on a listing of half a million entries: neither has to name every id.
 * Selections are immutable values; every function below returns a new one and never mutates.
 */
export type Selection =
	| { readonly kind: 'some'; readonly ids: ReadonlySet<EntryId> }
	| { readonly kind: 'allExcept'; readonly ids: ReadonlySet<EntryId> };

const NONE: ReadonlySet<EntryId> = new Set();

export const emptySelection: Selection = { kind: 'some', ids: NONE };

/** Everything in the listing, however large it grows or shrinks. */
export const everything: Selection = { kind: 'allExcept', ids: NONE };

export function selectOnly(id: EntryId): Selection {
	return { kind: 'some', ids: new Set([id]) };
}

export function selectIds(ids: Iterable<EntryId>): Selection {
	return { kind: 'some', ids: new Set(ids) };
}

export function isSelected(selection: Selection, id: EntryId): boolean {
	return selection.kind === 'some' ? selection.ids.has(id) : !selection.ids.has(id);
}

/**
 * How many entries are selected in a listing of `total`. An exclusion list can name ids the
 * listing no longer holds, so the count never goes below zero or above the total.
 */
export function selectedCount(selection: Selection, total: number): number {
	const count = selection.kind === 'some' ? selection.ids.size : total - selection.ids.size;
	return Math.max(0, Math.min(total, count));
}

/** Ctrl-click: flips one entry and leaves the rest alone. */
export function toggle(selection: Selection, id: EntryId): Selection {
	const ids = new Set(selection.ids);
	if (!ids.delete(id)) ids.add(id);
	return { kind: selection.kind, ids };
}

/** Adds entries to the selection (Ctrl+Shift-click extends a range without replacing). */
export function addIds(selection: Selection, added: Iterable<EntryId>): Selection {
	const ids = new Set(selection.ids);
	for (const id of added) {
		if (selection.kind === 'some') ids.add(id);
		else ids.delete(id);
	}
	return { kind: selection.kind, ids };
}

/** Swaps selected and unselected; O(1) because the id set is shared, not copied. */
export function invert(selection: Selection): Selection {
	return selection.kind === 'some'
		? { kind: 'allExcept', ids: selection.ids }
		: { kind: 'some', ids: selection.ids };
}

/**
 * Forgets ids that left the listing. An id that was excluded and is gone needs no exclusion, and a
 * selected one that is gone is no longer selected, so both kinds just drop it.
 */
export function removeIds(selection: Selection, removed: Iterable<EntryId>): Selection {
	let ids: Set<EntryId> | null = null;
	for (const id of removed) {
		if (!selection.ids.has(id)) continue;
		ids ??= new Set(selection.ids);
		ids.delete(id);
	}
	return ids ? { kind: selection.kind, ids } : selection;
}

/** Whether nothing is selected in a listing of `total`. */
export function isEmpty(selection: Selection, total: number): boolean {
	return selectedCount(selection, total) === 0;
}

/**
 * Collapses a selection that has become trivial: every entry excluded is nothing selected, and
 * everything selected by name (as many ids as entries) is the same as select all.
 */
export function normalise(selection: Selection, total: number): Selection {
	if (selection.kind === 'allExcept' && selection.ids.size >= total) return emptySelection;
	if (selection.kind === 'some' && total > 0 && selection.ids.size >= total) return everything;
	return selection;
}

/**
 * The view positions a range from `anchor` to `target` covers, inclusive and in either order, as
 * `[start, end)` for reading. Shift-click and Shift+arrow both select such a range.
 */
export function rangeBetween(anchor: number, target: number): [number, number] {
	return [Math.min(anchor, target), Math.max(anchor, target) + 1];
}
