// Tests the Shelf's grouping and the rows a keyboard walks
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ShelfItem } from '@liminal-hq/waypoint-protocol/generated/ShelfItem';
import { describe, expect, it } from 'vitest';
import { fileLocation } from '../services/fakeVfsClient';
import {
	commonOrigin,
	groupItems,
	groupKey,
	iconFor,
	itemKey,
	leafName,
	orderOf,
	visibleRows,
} from './shelfModel';

let next = 1;
function item(path: string): ShelfItem {
	const location = fileLocation(path);
	const cut = path.lastIndexOf('/');
	const id = next++;
	return {
		id,
		location,
		name: path.slice(cut + 1),
		addedMs: id,
		origin: fileLocation(cut <= 0 ? '/' : path.slice(0, cut)),
	};
}

describe('leafName', () => {
	it('is the last part of a path, and the whole of a root', () => {
		expect(leafName('/home/a/docs')).toBe('docs');
		expect(leafName('/home/a/docs/')).toBe('docs');
		expect(leafName('C:\\Users\\a')).toBe('a');
		expect(leafName('/')).toBe('/');
		expect(leafName('C:\\')).toBe('C:\\');
	});
});

describe('groupItems', () => {
	it('groups by origin, newest first, a group where its newest item is', () => {
		const a1 = item('/a/one');
		const b1 = item('/b/one');
		const a2 = item('/a/two');
		const groups = groupItems([a1, b1, a2]);
		expect(groups.map((g) => [g.name, g.items.map((i) => i.name)])).toEqual([
			['a', ['two', 'one']],
			['b', ['one']],
		]);
		expect(groups[0]?.origin).toEqual(fileLocation('/a'));
	});

	it('is empty for an empty Shelf', () => {
		expect(groupItems([])).toEqual([]);
	});
});

describe('visibleRows', () => {
	const items = [item('/a/one'), item('/a/two'), item('/b/three')];
	const groups = groupItems(items);

	it('lists each group header then its items', () => {
		const rows = visibleRows(groups, new Set());
		expect(rows.map((r) => r.key)).toEqual([
			groupKey(fileLocation('/b')),
			itemKey(items[2]!.id),
			groupKey(fileLocation('/a')),
			itemKey(items[1]!.id),
			itemKey(items[0]!.id),
		]);
		expect(orderOf(rows)).toEqual([items[2]!.id, items[1]!.id, items[0]!.id]);
	});

	it('leaves out the items of a collapsed group', () => {
		const rows = visibleRows(groups, new Set([fileLocation('/a').uri]));
		expect(rows.map((r) => r.kind)).toEqual(['group', 'item', 'group']);
		expect(rows[2]).toMatchObject({ kind: 'group', expanded: false });
	});
});

describe('the small rules', () => {
	it('draws a folder as a folder and everything else, known or not, as a page', () => {
		expect(iconFor('folder')).toBe('folder');
		expect(iconFor('file')).toBe('other');
		expect(iconFor('unknown')).toBe('other');
		expect(iconFor('missing')).toBe('other');
	});

	it('finds the one folder a set of items share, or none', () => {
		const a = item('/a/x');
		const a2 = item('/a/y');
		const b = item('/b/z');
		expect(commonOrigin([a, a2])).toEqual(fileLocation('/a'));
		expect(commonOrigin([a, b])).toBeNull();
		expect(commonOrigin([])).toBeNull();
	});
});
