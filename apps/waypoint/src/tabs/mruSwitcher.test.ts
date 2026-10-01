// Verifies the order the Ctrl+Tab walk visits tabs in, and stepping and ending a walk
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { afterEach, describe, expect, it } from 'vitest';
import { endSwitcher, stepSwitcher, switcherOrder, switcherWalk } from './mruSwitcher';

function snapshot(ids: number[], active: number | null, mru: number[]): SessionSnapshot {
	return {
		tabs: ids.map((id) => ({ id })),
		active,
		mru,
	} as unknown as SessionSnapshot;
}

afterEach(() => {
	endSwitcher();
});

describe('switcherOrder', () => {
	it('lists the active tab, then most recently used, then the rest in strip order', () => {
		expect(switcherOrder(snapshot([1, 2, 3, 4, 5], 3, [3, 5, 1]))).toEqual([3, 5, 1, 2, 4]);
	});

	it('puts the active tab first even when the MRU list does not', () => {
		expect(switcherOrder(snapshot([1, 2, 3], 2, [1, 2]))).toEqual([2, 1, 3]);
	});

	it('skips closed tabs and repeats', () => {
		expect(switcherOrder(snapshot([1, 2], 1, [9, 1, 2, 2]))).toEqual([1, 2]);
	});
});

describe('stepping', () => {
	const tabs = snapshot([1, 2, 3], 1, [1, 3, 2]);

	it('walks forward and wraps, so presses ping-pong from the active tab', () => {
		expect(stepSwitcher(tabs, 1)?.index).toBe(1);
		expect(stepSwitcher(tabs, 1)?.index).toBe(2);
		expect(stepSwitcher(tabs, 1)?.index).toBe(0);
	});

	it('walks backward from the active tab to the least recently used', () => {
		const walk = stepSwitcher(tabs, -1);
		expect(walk && walk.order[walk.index]).toBe(2);
	});

	it('ends on the candidate and clears the walk', () => {
		stepSwitcher(tabs, 1);
		expect(endSwitcher()).toBe(3);
		expect(switcherWalk()).toBeNull();
		expect(endSwitcher()).toBeNull();
	});

	it('does nothing with fewer than two tabs', () => {
		expect(stepSwitcher(snapshot([1], 1, [1]), 1)).toBeNull();
		expect(switcherWalk()).toBeNull();
	});
});
