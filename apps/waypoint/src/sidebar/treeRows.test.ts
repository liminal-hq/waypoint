// Verifies the Folders tree's flattening and its keyboard moves, without any rendering
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { fileLocation } from '../services/fakeVfsClient';
import { findRowByPrefix, flattenTree, folderRows, moveInTree, type TreeRow } from './treeRows';
import type { ChildrenState, FolderChild } from './folderTreeModel';

const node = (path: string, name = path.split('/').pop() || '/'): FolderChild => ({
	name,
	location: fileLocation(path),
});
const uri = (path: string) => fileLocation(path).uri;

const root = node('/', '/');
const kids: Record<string, ChildrenState> = {
	[uri('/')]: {
		status: 'ready',
		total: 2,
		children: [node('/home'), node('/usr')],
	},
	[uri('/home')]: { status: 'ready', total: 1, children: [node('/home/ann')] },
	[uri('/usr')]: { status: 'ready', total: 0, children: [] },
};
const childrenOf = (key: string) => kids[key];

describe('flattenTree', () => {
	it('shows only the root while nothing is expanded', () => {
		const rows = flattenTree(root, new Set(), childrenOf);
		expect(rows).toHaveLength(1);
		expect(rows[0]).toMatchObject({ level: 1, expanded: false, expandable: true, loading: false });
	});

	it('lists the children of expanded nodes only, with level, parent and place among siblings', () => {
		const rows = folderRows(flattenTree(root, new Set([uri('/'), uri('/home')]), childrenOf));
		expect(rows.map((row) => [row.name, row.level])).toEqual([
			['/', 1],
			['home', 2],
			['ann', 3],
			['usr', 2],
		]);
		const usr = rows.find((row) => row.name === 'usr')!;
		expect(usr).toMatchObject({ parentKey: uri('/'), position: 2, siblings: 2 });
	});

	it('marks an expanded node loading until its children are known, and a childless one as a leaf', () => {
		const loading = folderRows(
			flattenTree(root, new Set([uri('/')]), () => ({ status: 'loading' })),
		);
		expect(loading[0]).toMatchObject({ expanded: true, loading: true, expandable: true });
		const leaf = folderRows(flattenTree(root, new Set([uri('/'), uri('/usr')]), childrenOf));
		expect(leaf.find((row) => row.name === 'usr')).toMatchObject({ expandable: false });
	});

	it('adds a note row when a node has more children than it shows', () => {
		const capped: ChildrenState = { status: 'ready', total: 5, children: [node('/a'), node('/b')] };
		const entries = flattenTree(root, new Set([uri('/')]), () => capped);
		expect(entries[entries.length - 1]).toMatchObject({
			kind: 'more',
			shown: 2,
			total: 5,
			level: 2,
		});
		expect(folderRows(entries)).toHaveLength(3);
	});
});

describe('moveInTree', () => {
	const rows: TreeRow[] = folderRows(
		flattenTree(root, new Set([uri('/'), uri('/home')]), childrenOf),
	);
	const key = (name: string) => rows.find((row) => row.name === name)!.key;

	it('steps and jumps between visible rows, and stops at the ends', () => {
		expect(moveInTree(rows, key('home'), 'ArrowDown')).toEqual({ focus: key('ann') });
		expect(moveInTree(rows, key('home'), 'ArrowUp')).toEqual({ focus: key('/') });
		expect(moveInTree(rows, key('/'), 'ArrowUp')).toEqual({});
		expect(moveInTree(rows, key('usr'), 'ArrowDown')).toEqual({});
		expect(moveInTree(rows, key('ann'), 'Home')).toEqual({ focus: key('/') });
		expect(moveInTree(rows, key('home'), 'End')).toEqual({ focus: key('usr') });
	});

	it('opens a closed node on Right, then enters it; closes an open one on Left, then goes to its parent', () => {
		expect(moveInTree(rows, key('ann'), 'ArrowRight')).toEqual({
			expand: { key: key('ann'), expanded: true },
		});
		expect(moveInTree(rows, key('home'), 'ArrowRight')).toEqual({ focus: key('ann') });
		expect(moveInTree(rows, key('home'), 'ArrowLeft')).toEqual({
			expand: { key: key('home'), expanded: false },
		});
		expect(moveInTree(rows, key('ann'), 'ArrowLeft')).toEqual({ focus: key('home') });
		expect(moveInTree(rows, key('/'), 'ArrowLeft')).toEqual({
			expand: { key: key('/'), expanded: false },
		});
	});

	it('does nothing on Right for a leaf, and for an unknown row', () => {
		const withLeaf = folderRows(
			flattenTree(root, new Set([uri('/'), uri('/home'), uri('/usr')]), childrenOf),
		);
		const leaf = withLeaf.find((row) => row.name === 'usr')!;
		expect(moveInTree(withLeaf, leaf.key, 'ArrowRight')).toEqual({});
		expect(moveInTree(rows, 'nope', 'ArrowDown')).toEqual({});
	});
});

describe('findRowByPrefix', () => {
	const rows = folderRows(flattenTree(root, new Set([uri('/'), uri('/home')]), childrenOf));
	it('finds the next row starting with the text, ignoring case, and wraps round', () => {
		expect(findRowByPrefix(rows, rows[0]!.key, 'U')).toBe(rows[3]!.key);
		expect(findRowByPrefix(rows, rows[3]!.key, 'h')).toBe(rows[1]!.key);
	});
	it('moves on from the current row for one letter and stays for a longer prefix', () => {
		expect(findRowByPrefix(rows, rows[1]!.key, 'h')).toBe(rows[1]!.key);
		expect(findRowByPrefix(rows, rows[1]!.key, 'ho')).toBe(rows[1]!.key);
	});
	it('finds nothing for no match or no text', () => {
		expect(findRowByPrefix(rows, undefined, 'zzz')).toBeNull();
		expect(findRowByPrefix(rows, undefined, '')).toBeNull();
	});
});
