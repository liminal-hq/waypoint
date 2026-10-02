// Tests the Shelf store: revision gating, pruning, selection, collapsing and height
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ShelfItem } from '@liminal-hq/waypoint-protocol/generated/ShelfItem';
import { describe, expect, it } from 'vitest';
import { fileLocation } from '../services/fakeVfsClient';
import {
	clampHeight,
	createShelfStore,
	DEFAULT_HEIGHT,
	MAX_HEIGHT,
	MIN_HEIGHT,
} from './shelfStore';

const make = (id: number, path = `/d/f${id}`): ShelfItem => ({
	id,
	location: fileLocation(path),
	name: `f${id}`,
	addedMs: id,
	origin: fileLocation('/d'),
});

describe('sync', () => {
	it('takes a newer Shelf and ignores an older one', () => {
		const store = createShelfStore();
		store.getState().sync([make(1)], 5);
		store.getState().sync([make(1), make(2)], 9);
		store.getState().sync([make(1)], 7);
		expect(store.getState().items.map((i) => i.id)).toEqual([1, 2]);
		expect(store.getState().revision).toBe(9);
	});

	it('takes the same revision again (a replay changes nothing but is not refused)', () => {
		const store = createShelfStore();
		store.getState().sync([make(1)], 4);
		store.getState().sync([make(1), make(2)], 4);
		expect(store.getState().items).toHaveLength(2);
	});

	it('forgets the selection, anchor, focus and file facts of items that left', () => {
		const store = createShelfStore();
		store.getState().sync([make(1), make(2)], 1);
		store.getState().select(1, 'only', [1, 2]);
		store.getState().select(2, 'toggle', [1, 2]);
		store.getState().setStatus(new Map([[make(1).location.uri, 'missing']]));
		store.getState().setFocus('item:1');
		store.getState().sync([make(2)], 2);
		const state = store.getState();
		expect([...state.selected]).toEqual([2]);
		expect(state.focus).toBeNull();
		expect(state.status.size).toBe(0);
	});
});

describe('selection', () => {
	const order = [3, 2, 1];
	const seeded = () => {
		const store = createShelfStore();
		store.getState().sync([make(1), make(2), make(3)], 1);
		return store;
	};

	it('selects one, toggles one, and extends a range from the anchor', () => {
		const store = seeded();
		store.getState().select(3, 'only', order);
		expect([...store.getState().selected]).toEqual([3]);
		store.getState().select(1, 'toggle', order);
		expect([...store.getState().selected].sort()).toEqual([1, 3]);
		store.getState().select(1, 'toggle', order);
		expect([...store.getState().selected]).toEqual([3]);
		store.getState().select(3, 'only', order);
		store.getState().select(1, 'range', order);
		expect([...store.getState().selected].sort()).toEqual([1, 2, 3]);
		// A range keeps its anchor, so it can shrink back.
		store.getState().select(2, 'range', order);
		expect([...store.getState().selected].sort()).toEqual([2, 3]);
	});

	it('selects everything, and clears', () => {
		const store = seeded();
		store.getState().selectAll(order);
		expect(store.getState().selected.size).toBe(3);
		store.getState().clearSelection();
		expect(store.getState().selected.size).toBe(0);
	});
});

describe('the panel’s own state', () => {
	it('opens, closes and toggles', () => {
		const store = createShelfStore();
		expect(store.getState().open).toBe(false);
		store.getState().toggleOpen();
		expect(store.getState().open).toBe(true);
		store.getState().setOpen(false);
		expect(store.getState().open).toBe(false);
	});

	it('collapses and expands a group', () => {
		const store = createShelfStore();
		store.getState().toggleGroup('file:///d');
		expect(store.getState().collapsed.has('file:///d')).toBe(true);
		store.getState().toggleGroup('file:///d', false);
		expect(store.getState().collapsed.has('file:///d')).toBe(false);
	});

	it('keeps the height between its limits', () => {
		const store = createShelfStore();
		expect(store.getState().height).toBe(DEFAULT_HEIGHT);
		store.getState().setHeight(10);
		expect(store.getState().height).toBe(MIN_HEIGHT);
		store.getState().setHeight(5000);
		expect(store.getState().height).toBe(MAX_HEIGHT);
		expect(clampHeight(333.4)).toBe(333);
	});

	it('records file facts, replacing and leaving the rest, and counts focus requests', () => {
		const store = createShelfStore();
		store.getState().setStatus(new Map([['a', 'file']]));
		store.getState().setStatus(new Map([['b', 'missing']]));
		store.getState().setStatus(new Map([['a', 'folder']]));
		expect([...store.getState().status]).toEqual([
			['a', 'folder'],
			['b', 'missing'],
		]);
		store.getState().requestFocus();
		expect(store.getState().focusRequests).toBe(1);
	});
});
