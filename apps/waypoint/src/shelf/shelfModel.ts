// The Shelf as the panel shows it: items grouped by the folder they came from, newest first, and the rows a keyboard walks
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ShelfItem } from '@liminal-hq/waypoint-protocol/generated/ShelfItem';
import type { ShelfItemId } from '@liminal-hq/waypoint-protocol/generated/ShelfItemId';

/** What the panel knows of an item's file: nothing yet, a file, a folder, or that it is not there any more. */
export type ItemState = 'unknown' | 'file' | 'folder' | 'missing';

/** One origin folder and its items, newest first. */
export interface ShelfGroup {
	origin: Location;
	/** The folder's own name; a root keeps its whole path. */
	name: string;
	items: ShelfItem[];
}

/** A row the keyboard can land on: a group's header or one of its items. */
export type ShelfRow =
	| { kind: 'group'; key: string; group: ShelfGroup; expanded: boolean }
	| { kind: 'item'; key: string; item: ShelfItem; group: ShelfGroup };

export const groupKey = (origin: Location) => `group:${origin.uri}`;
export const itemKey = (id: ShelfItemId) => `item:${id}`;

/** The last part of a display path, or the whole of a root. */
export function leafName(display: string): string {
	const trimmed = display.replace(/[/\\]+$/, '');
	const at = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
	return at >= 0 && at < trimmed.length - 1 ? trimmed.slice(at + 1) : display;
}

/**
 * The groups the panel lists. Items are newest first (the model keeps insertion order, oldest
 * first), and a group sits where its newest item does, so what was just added is at the top.
 */
export function groupItems(items: readonly ShelfItem[]): ShelfGroup[] {
	const groups = new Map<string, ShelfGroup>();
	for (const item of [...items].reverse()) {
		const key = item.origin.uri;
		let group = groups.get(key);
		if (!group) {
			group = { origin: item.origin, name: leafName(item.origin.display), items: [] };
			groups.set(key, group);
		}
		group.items.push(item);
	}
	return [...groups.values()];
}

/** Every row the panel shows, in order, with a collapsed group's items left out. */
export function visibleRows(
	groups: readonly ShelfGroup[],
	collapsed: ReadonlySet<string>,
): ShelfRow[] {
	return groups.flatMap((group): ShelfRow[] => {
		const expanded = !collapsed.has(group.origin.uri);
		return [
			{ kind: 'group', key: groupKey(group.origin), group, expanded },
			...(expanded
				? group.items.map((item): ShelfRow => ({
						kind: 'item',
						key: itemKey(item.id),
						item,
						group,
					}))
				: []),
		];
	});
}

/** The ids of the items in the rows, in the order shown. */
export function orderOf(rows: readonly ShelfRow[]): ShelfItemId[] {
	return rows.flatMap((row) => (row.kind === 'item' ? [row.item.id] : []));
}

/** The icon group of an item: a folder when it is one, a plain page otherwise (and until it is known). */
export function iconFor(state: ItemState): IconGroup {
	return state === 'folder' ? 'folder' : 'other';
}

/** The folder every one of `items` sits in, or `null` when they come from more than one. */
export function commonOrigin(items: readonly ShelfItem[]): Location | null {
	const first = items[0];
	if (!first) return null;
	return items.every((item) => item.origin.uri === first.origin.uri) ? first.origin : null;
}

/** Where a row sits on screen, as far as moving between the strip's lines needs. */
export interface RowBox {
	top: number;
	left: number;
	width: number;
}

/** Rows whose tops differ by less than this are on one line of the strip. */
const SAME_LINE = 4;

/**
 * The row Up (`-1`) or Down (`1`) lands on in a strip that wraps: on the nearest line above or
 * below the row at `at`, the one whose centre is closest to its own. `null` when there is no such
 * line (a strip on one line has no Up or Down).
 */
export function rowOnAdjacentLine(
	boxes: readonly RowBox[],
	at: number,
	direction: 1 | -1,
): number | null {
	const here = boxes[at];
	if (!here) return null;
	let lineTop: number | null = null;
	boxes.forEach((box) => {
		const delta = (box.top - here.top) * direction;
		if (delta >= SAME_LINE && (lineTop === null || (box.top - lineTop) * direction < 0)) {
			lineTop = box.top;
		}
	});
	if (lineTop === null) return null;
	const centre = here.left + here.width / 2;
	let best: number | null = null;
	let bestGap = Infinity;
	boxes.forEach((box, index) => {
		if (Math.abs(box.top - (lineTop as number)) >= SAME_LINE) return;
		const gap = Math.abs(box.left + box.width / 2 - centre);
		if (gap < bestGap) {
			best = index;
			bestGap = gap;
		}
	});
	return best;
}
