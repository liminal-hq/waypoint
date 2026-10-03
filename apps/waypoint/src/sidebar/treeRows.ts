// Flattens the Folders tree into the rows it shows, and finds the row a key press moves to
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import type { ChildrenState, FolderChild } from './folderTreeModel';

export interface TreeRow {
	kind: 'folder';
	/** The location's `uri`, which identifies the node. */
	key: string;
	name: string;
	location: Location;
	/** Which standard folder of the user's this is, when it is one. */
	special?: SpecialFolder;
	/** 1 for the root. */
	level: number;
	expanded: boolean;
	/** False once the folder is known to have no sub-folders; every other folder shows it can open. */
	expandable: boolean;
	loading: boolean;
	parentKey: string | null;
	/** Place among siblings, 1-based, for `aria-posinset` (0 while the siblings are not known). */
	position: number;
	siblings: number;
}

/** Stands in for the folders a node has but does not show. */
export interface TreeNote {
	kind: 'more';
	key: string;
	level: number;
	shown: number;
	total: number;
}

export type TreeEntry = TreeRow | TreeNote;

/**
 * The rows in display order: the root, and below every expanded node its children. A collapsed
 * node's children are not rows, so they are not in the DOM either.
 */
export function flattenTree(
	root: FolderChild,
	expanded: ReadonlySet<string>,
	childrenOf: (uri: string) => ChildrenState | undefined,
): TreeEntry[] {
	const entries: TreeEntry[] = [];
	const visit = (
		node: FolderChild,
		level: number,
		parentKey: string | null,
		position: number,
		siblings: number,
	) => {
		const uri = node.location.uri;
		const open = expanded.has(uri);
		const state = open ? childrenOf(uri) : undefined;
		const known = state?.status === 'ready' ? state : null;
		entries.push({
			kind: 'folder',
			key: uri,
			name: node.name,
			location: node.location,
			special: node.special,
			level,
			expanded: open,
			expandable: !(known && known.total === 0),
			loading: open && (state === undefined || state.status === 'loading'),
			parentKey,
			position,
			siblings,
		});
		if (!open || !known) return;
		known.children.forEach((child, index) =>
			visit(child, level + 1, uri, index + 1, known.children.length),
		);
		if (known.total > known.children.length) {
			entries.push({
				kind: 'more',
				key: `${uri}#more`,
				level: level + 1,
				shown: known.children.length,
				total: known.total,
			});
		}
	};
	visit(root, 1, null, 1, 1);
	return entries;
}

/** The folder rows only: what arrow keys and type-ahead move between. */
export function folderRows(entries: readonly TreeEntry[]): TreeRow[] {
	return entries.filter((entry): entry is TreeRow => entry.kind === 'folder');
}

export type TreeKey = 'ArrowDown' | 'ArrowUp' | 'ArrowRight' | 'ArrowLeft' | 'Home' | 'End';

export interface TreeMove {
	/** The row to focus, when the key moves focus. */
	focus?: string;
	/** A node to expand or collapse, when the key does that. */
	expand?: { key: string; expanded: boolean };
}

/**
 * What a navigation key does from the row at `from` (WAI-ARIA tree pattern): Down and Up step
 * through the visible rows; Right opens a closed node, then enters an open one; Left closes an
 * open node, then goes to its parent; Home and End go to the first and last visible row.
 */
export function moveInTree(rows: readonly TreeRow[], from: string, key: TreeKey): TreeMove {
	const index = rows.findIndex((row) => row.key === from);
	const row = rows[index];
	if (!row) return {};
	switch (key) {
		case 'ArrowDown':
			return rows[index + 1] ? { focus: rows[index + 1]!.key } : {};
		case 'ArrowUp':
			return rows[index - 1] ? { focus: rows[index - 1]!.key } : {};
		case 'Home':
			return { focus: rows[0]!.key };
		case 'End':
			return { focus: rows[rows.length - 1]!.key };
		case 'ArrowRight': {
			if (!row.expandable) return {};
			if (!row.expanded) return { expand: { key: row.key, expanded: true } };
			const child = rows[index + 1];
			return child && child.parentKey === row.key ? { focus: child.key } : {};
		}
		case 'ArrowLeft':
			if (row.expanded && row.expandable) return { expand: { key: row.key, expanded: false } };
			return row.parentKey ? { focus: row.parentKey } : {};
	}
}

/** The first row after `from` (wrapping round) whose name starts with `prefix`, ignoring case. */
export function findRowByPrefix(
	rows: readonly TreeRow[],
	from: string | undefined,
	prefix: string,
): string | null {
	if (rows.length === 0 || prefix === '') return null;
	const needle = prefix.toLocaleLowerCase();
	const start = Math.max(
		0,
		rows.findIndex((row) => row.key === from),
	);
	// A repeated single letter cycles through the matches; a longer prefix may stay where it is.
	const first = prefix.length === 1 ? 1 : 0;
	for (let offset = first; offset < rows.length + first; offset++) {
		const row = rows[(start + offset) % rows.length]!;
		if (row.name.toLocaleLowerCase().startsWith(needle)) return row.key;
	}
	return null;
}
