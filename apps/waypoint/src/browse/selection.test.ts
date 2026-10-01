// Verifies the selection model's pure functions, including its O(1) whole-listing forms
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	addIds,
	emptySelection,
	everything,
	invert,
	isEmpty,
	isSelected,
	normalise,
	rangeBetween,
	removeIds,
	selectedCount,
	selectIds,
	selectOnly,
	toggle,
} from './selection';

describe('selection', () => {
	it('starts empty', () => {
		expect(selectedCount(emptySelection, 10)).toBe(0);
		expect(isEmpty(emptySelection, 10)).toBe(true);
		expect(isSelected(emptySelection, 3)).toBe(false);
	});

	it('replaces the selection on a plain click', () => {
		const selection = selectOnly(7);
		expect(isSelected(selection, 7)).toBe(true);
		expect(isSelected(selection, 8)).toBe(false);
		expect(selectedCount(selection, 10)).toBe(1);
	});

	it('toggles one entry in and out, leaving the others', () => {
		let selection = selectIds([1, 2]);
		selection = toggle(selection, 3);
		expect([...selection.ids].sort()).toEqual([1, 2, 3]);
		selection = toggle(selection, 1);
		expect([...selection.ids].sort()).toEqual([2, 3]);
	});

	it('never mutates the selection it was given', () => {
		const before = selectIds([1, 2]);
		toggle(before, 9);
		addIds(before, [9]);
		removeIds(before, [1]);
		expect([...before.ids]).toEqual([1, 2]);
	});

	it('selects everything without naming an id, at any size', () => {
		expect(selectedCount(everything, 500_000)).toBe(500_000);
		expect(isSelected(everything, 499_999)).toBe(true);
		expect(everything.ids.size).toBe(0);
	});

	it('toggles an entry out of everything as an exclusion', () => {
		const selection = toggle(everything, 42);
		expect(selection.kind).toBe('allExcept');
		expect(isSelected(selection, 42)).toBe(false);
		expect(isSelected(selection, 43)).toBe(true);
		expect(selectedCount(selection, 500_000)).toBe(499_999);
		expect(isSelected(toggle(selection, 42), 42)).toBe(true);
	});

	it('inverts in constant time by sharing the id set', () => {
		const some = selectIds([1, 2, 3]);
		const flipped = invert(some);
		expect(flipped.kind).toBe('allExcept');
		expect(flipped.ids).toBe(some.ids);
		expect(selectedCount(flipped, 10)).toBe(7);
		expect(invert(flipped).kind).toBe('some');
		expect(selectedCount(invert(emptySelection), 10)).toBe(10);
		expect(selectedCount(invert(everything), 10)).toBe(0);
	});

	it('adds ids to a selection and removes exclusions from everything', () => {
		expect(selectedCount(addIds(selectIds([1]), [2, 3]), 10)).toBe(3);
		const excluded = toggle(toggle(everything, 1), 2);
		const restored = addIds(excluded, [1]);
		expect(isSelected(restored, 1)).toBe(true);
		expect(isSelected(restored, 2)).toBe(false);
	});

	it('forgets removed ids in both forms', () => {
		expect(selectedCount(removeIds(selectIds([1, 2, 3]), [2, 9]), 10)).toBe(2);
		const excluded = toggle(everything, 5);
		expect(selectedCount(excluded, 10)).toBe(9);
		// Entry 5 was removed from the listing (now 9 entries): nothing is excluded any more.
		expect(selectedCount(removeIds(excluded, [5]), 9)).toBe(9);
	});

	it('returns the same selection when a removal touches nothing', () => {
		const selection = selectIds([1, 2]);
		expect(removeIds(selection, [8, 9])).toBe(selection);
	});

	it('never counts past the listing it is in', () => {
		expect(selectedCount(selectIds([1, 2, 3, 4]), 2)).toBe(2);
		expect(selectedCount(toggle(everything, 1), 0)).toBe(0);
	});

	it('normalises the trivial forms', () => {
		expect(normalise(selectIds([1, 2, 3]), 3)).toBe(everything);
		expect(normalise(invert(selectIds([1, 2, 3])), 3)).toBe(emptySelection);
		const partial = selectIds([1]);
		expect(normalise(partial, 3)).toBe(partial);
		expect(normalise(emptySelection, 0)).toBe(emptySelection);
	});

	it('gives a range between positions in either order', () => {
		expect(rangeBetween(2, 5)).toEqual([2, 6]);
		expect(rangeBetween(5, 2)).toEqual([2, 6]);
		expect(rangeBetween(4, 4)).toEqual([4, 5]);
	});
});
