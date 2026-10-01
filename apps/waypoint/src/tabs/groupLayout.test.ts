// Verifies the strip's render order for groups, and where moves land around group boundaries
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Group } from '@liminal-hq/waypoint-protocol/generated/Group';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { describe, expect, it } from 'vitest';
import {
	buildStrip,
	groupStepTarget,
	hiddenActiveGroup,
	landingIndex,
	settledOrder,
	stepTarget,
} from './groupLayout';

function tab(id: number, group: number | null = null, pinned = false): TabSnapshot {
	return {
		id,
		location: { uri: `file:///t${id}`, display: `/t${id}` },
		back: [],
		forward: [],
		pinned,
		colour: null,
		group,
		hints: {},
	} as unknown as TabSnapshot;
}

const group = (id: number, collapsed = false): Group => ({
	id,
	name: `G${id}`,
	colour: null,
	collapsed,
});

const ids = (tabs: readonly TabSnapshot[]) => tabs.map((entry) => entry.id);

describe('buildStrip', () => {
	const snapshot = (tabs: TabSnapshot[], groups: Group[], active = 1): SessionSnapshot =>
		({ tabs, groups, active, pairs: [], mru: [], revision: 1 }) as unknown as SessionSnapshot;

	it('puts each group chip before its tabs and marks the bracket ends', () => {
		const { items } = buildStrip(snapshot([tab(1), tab(2, 1), tab(3, 1), tab(4)], [group(1)]));
		expect(
			items.map((item) => (item.kind === 'chip' ? `chip${item.group.id}` : item.tab.id)),
		).toEqual([1, 'chip1', 2, 3, 4]);
		const [, , two, three] = items;
		expect(two).toMatchObject({ groupFirst: true, groupLast: false });
		expect(three).toMatchObject({ groupFirst: false, groupLast: true });
	});

	it('hides the tabs of a collapsed group but keeps its chip and member count', () => {
		const { items } = buildStrip(snapshot([tab(1, 1), tab(2, 1), tab(3)], [group(1, true)]));
		expect(items).toHaveLength(2);
		expect(items[0]).toMatchObject({ kind: 'chip', containsActive: true });
		expect(items[0]?.kind === 'chip' && items[0].members).toHaveLength(2);
	});

	it('counts a pinned group chip and its tabs as sticky slots', () => {
		const { items, pinSlots } = buildStrip(
			snapshot([tab(1, 1, true), tab(2, 1, true), tab(3, null, true), tab(4)], [group(1)]),
		);
		expect(pinSlots).toBe(4);
		expect(items.map((item) => item.pinIndex)).toEqual([0, 1, 2, 3, -1]);
	});

	it('draws a tab whose group is unknown as an ordinary tab', () => {
		const { items } = buildStrip(snapshot([tab(1, 9)], []));
		expect(items).toHaveLength(1);
		expect(items[0]).toMatchObject({ kind: 'tab', group: null });
	});

	it('names the collapsed group that hides the active tab', () => {
		const tabs = [tab(1, 1), tab(2)];
		expect(hiddenActiveGroup(snapshot(tabs, [group(1, true)], 1))).toBe(1);
		expect(hiddenActiveGroup(snapshot(tabs, [group(1, false)], 1))).toBeNull();
		expect(hiddenActiveGroup(snapshot(tabs, [group(1, true)], 2))).toBeNull();
	});
});

describe('where a move lands', () => {
	// 1 | G1: 2 3 4 | 5 | G2: 6 7
	const tabs = [tab(1), tab(2, 1), tab(3, 1), tab(4, 1), tab(5), tab(6, 2), tab(7, 2)];

	it('keeps a grouped tab inside its group however far it is dragged', () => {
		expect(landingIndex(tabs, 1, 0)).toBe(1);
		expect(landingIndex(tabs, 1, 6)).toBe(3);
		expect(landingIndex(tabs, 2, 1)).toBe(1);
	});

	it('lets a grouped tab move between two tabs of its own group', () => {
		expect(ids(settledOrder(tabs, [2], 2, 1))).toEqual([1, 3, 2, 4, 5, 6, 7]);
	});

	it('settles a tab dropped inside another group to the group boundary', () => {
		// Dropped between 6 and 7 it lands after the group, which stays whole.
		const order = settledOrder(tabs, [1], 5, null);
		expect(ids(order)).toEqual([2, 3, 4, 5, 6, 7, 1]);
		expect(landingIndex(tabs, 4, 2)).toBe(4);
	});

	it('lets a tab dropped before a group stay before it', () => {
		expect(landingIndex(tabs, 4, 1)).toBe(1);
	});

	it('keeps a pinned tab among the pinned and an unpinned one among the rest', () => {
		const mixed = [tab(1, null, true), tab(2, null, true), tab(3), tab(4)];
		expect(landingIndex(mixed, 0, 3)).toBe(1);
		expect(landingIndex(mixed, 3, 0)).toBe(2);
	});

	it('steps an ungrouped tab over a whole neighbouring group', () => {
		expect(stepTarget(tabs, 0, 1)).toBe(3);
		expect(stepTarget(tabs, 4, -1)).toBe(1);
		expect(stepTarget(tabs, 4, 1)).toBe(6);
	});

	it('does not let a keyboard step take a tab out of its group', () => {
		expect(stepTarget(tabs, 3, 1)).toBe(3);
		expect(stepTarget(tabs, 1, -1)).toBe(1);
		expect(stepTarget(tabs, 1, 1)).toBe(2);
	});

	it('steps a whole group past a tab or a neighbouring group, and stops at the ends', () => {
		expect(groupStepTarget(tabs, 1, 1)).toBe(2);
		expect(groupStepTarget(tabs, 1, -1)).toBe(0);
		expect(groupStepTarget(tabs, 2, -1)).toBe(4);
		expect(groupStepTarget(tabs, 2, 1)).toBeNull();
		const groups = [tab(1, 1), tab(2, 1), tab(3, 2)];
		expect(groupStepTarget(groups, 1, 1)).toBe(1);
		expect(groupStepTarget(groups, 1, -1)).toBeNull();
	});
});
