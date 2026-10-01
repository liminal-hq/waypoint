// Verifies the pane helpers: equal shares, divider moves that keep the total, visibility and pane order
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import { describe, expect, it } from 'vitest';
import {
	equalSizes,
	MIN_PANE_SHARE,
	moveDivider,
	paneAfter,
	pairOfTab,
	sameSizes,
	visibleTabs,
} from './pairLayout';
import { FakeTabsStore } from '../services/fakeTabsStore';

const pair: Pair = {
	id: 1,
	panes: [4, 5, 6],
	layout: 'sideBySide',
	sizes: [334, 333, 333],
	origin: { kind: 'joined' },
};

describe('equalSizes', () => {
	it('adds up to 1000 and gives the remainder to the first panes', () => {
		expect(equalSizes(2)).toEqual([500, 500]);
		expect(equalSizes(3)).toEqual([334, 333, 333]);
		expect(equalSizes(0)).toEqual([]);
		for (let n = 1; n < 9; n++) expect(equalSizes(n).reduce((a, b) => a + b, 0)).toBe(1000);
	});
});

describe('moveDivider', () => {
	it('moves share from one side of the divider to the other and keeps the total', () => {
		expect(moveDivider([500, 500], 0, 120)).toEqual([620, 380]);
		expect(moveDivider([334, 333, 333], 1, -50)).toEqual([334, 283, 383]);
	});

	it('never takes a pane under the minimum', () => {
		expect(moveDivider([500, 500], 0, 900)).toEqual([1000 - MIN_PANE_SHARE, MIN_PANE_SHARE]);
		expect(moveDivider([500, 500], 0, -900)).toEqual([MIN_PANE_SHARE, 1000 - MIN_PANE_SHARE]);
	});

	it('ignores a divider that does not exist', () => {
		expect(moveDivider([500, 500], 1, 50)).toEqual([500, 500]);
	});

	it('compares sizes', () => {
		expect(sameSizes([500, 500], [500, 500])).toBe(true);
		expect(sameSizes([500, 500], [400, 600])).toBe(false);
	});
});

describe('paneAfter and pairOfTab', () => {
	it('wraps round the ends', () => {
		expect(paneAfter(pair, 4, 1)).toBe(5);
		expect(paneAfter(pair, 6, 1)).toBe(4);
		expect(paneAfter(pair, 4, -1)).toBe(6);
		expect(paneAfter(pair, 9, 1)).toBeUndefined();
	});

	it('finds the pair a tab is in', () => {
		expect(pairOfTab([pair], 5)).toBe(pair);
		expect(pairOfTab([pair], 1)).toBeUndefined();
		expect(pairOfTab([pair], null)).toBeUndefined();
	});
});

describe('visibleTabs', () => {
	it('is the active tab, or every pane of its pair', async () => {
		const store = FakeTabsStore.singleWindow();
		const snapshot = store.snapshot('main-1');
		expect(visibleTabs(null).size).toBe(0);
		expect(visibleTabs(snapshot).size).toBe(0);
		expect([...visibleTabs({ ...snapshot, active: 5, pairs: [pair] })]).toEqual([4, 5, 6]);
		expect([...visibleTabs({ ...snapshot, active: 9, pairs: [pair] })]).toEqual([9]);
	});
});
