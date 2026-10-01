// Verifies the drag geometry: the window's bounds, the strip's band, body hits, previews and where a unit lands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { describe, expect, it } from 'vitest';
import {
	bodyAt,
	distanceFromStrip,
	outsideWindow,
	planReorder,
	previewShifts,
	slotLeft,
} from './dragLayout';
import { unitDropIndex } from './reorder';

const tab = (id: number, extra: Partial<TabSnapshot> = {}): TabSnapshot => ({
	id,
	location: { display: `/${id}`, uri: `file:///${id}` },
	back: [],
	forward: [],
	pinned: false,
	colour: null,
	group: null,
	hints: { scrollTop: 0, focused: null },
	...extra,
});

// Four tabs, 100 wide, side by side.
const spans = [0, 100, 200, 300].map((left) => ({ left, right: left + 100 }));
const tabs = [tab(1), tab(2), tab(3), tab(4)];

describe('outsideWindow', () => {
	const view = { width: 1000, height: 700 };
	it('is true once a coordinate is past the client area on any side', () => {
		expect(outsideWindow({ x: 0, y: 0 }, view)).toBe(false);
		expect(outsideWindow({ x: 999, y: 699 }, view)).toBe(false);
		expect(outsideWindow({ x: -1, y: 300 }, view)).toBe(true);
		expect(outsideWindow({ x: 300, y: -1 }, view)).toBe(true);
		expect(outsideWindow({ x: 1000, y: 300 }, view)).toBe(true);
		expect(outsideWindow({ x: 300, y: 700 }, view)).toBe(true);
	});
});

describe('distanceFromStrip', () => {
	const strip = { left: 0, top: 10, right: 500, bottom: 40 };
	it('is zero inside the strip and counts from its nearer edge outside', () => {
		expect(distanceFromStrip(strip, 25)).toBe(0);
		expect(distanceFromStrip(strip, 64)).toBe(24);
		expect(distanceFromStrip(strip, -10)).toBe(20);
	});
});

describe('unitDropIndex', () => {
	it('counts the tabs outside the unit whose centre the unit’s centre has passed', () => {
		expect(unitDropIndex(spans, [0, 1], 260)).toBe(1);
		expect(unitDropIndex(spans, [0, 1], 360)).toBe(2);
		expect(unitDropIndex(spans, [1, 2], 0)).toBe(0);
	});
});

describe('bodyAt', () => {
	it('matches the middle half of a tab, following how far it has slid, and skips the dragged ones', () => {
		const none = new Map<number, number>();
		expect(bodyAt(tabs, spans, none, new Set([2]), 250)).toBe(3);
		expect(bodyAt(tabs, spans, none, new Set([2]), 215)).toBeNull();
		expect(bodyAt(tabs, spans, none, new Set([2]), 150)).toBeNull();
		// Tab 4 has slid 200 to the left, so its body is now 125..175.
		expect(bodyAt(tabs, spans, new Map([[4, -200]]), new Set([2]), 150)).toBe(4);
	});

	it('ignores hidden tabs, which have no extent of their own', () => {
		const flat = [{ left: 0, right: 0 }, ...spans.slice(1)];
		expect(bodyAt(tabs, flat, new Map(), new Set(), 0)).toBeNull();
	});
});

describe('planReorder', () => {
	const measure = { spans, chips: [] };
	it('lands a tab where its centre is, and previews the slides', () => {
		const plan = planReorder(tabs, [], [1], measure, 260, true);
		expect(plan.to).toBe(2);
		expect(plan.order.map((t) => t.id)).toEqual([2, 3, 1, 4]);
		expect(previewShifts(tabs, [1], plan.order, 100)).toEqual(
			new Map([
				[2, -100],
				[3, -100],
			]),
		);
	});

	it('moves the other way too', () => {
		const plan = planReorder(tabs, [], [4], measure, 50, true);
		expect(plan.to).toBe(0);
		expect(previewShifts(tabs, [4], plan.order, 100)).toEqual(
			new Map([
				[1, 100],
				[2, 100],
				[3, 100],
			]),
		);
	});

	it('keeps a pinned tab among the pinned ones and an unpinned tab out of them', () => {
		const pinned = [tab(1, { pinned: true }), tab(2, { pinned: true }), tab(3), tab(4)];
		expect(planReorder(pinned, [], [1], measure, 900, true).to).toBe(1);
		expect(planReorder(pinned, [], [4], measure, -900, true).to).toBe(2);
	});

	it('takes a pair’s panes together', () => {
		const pair: Pair = {
			id: 1,
			panes: [1, 2],
			layout: 'sideBySide',
			sizes: [500, 500],
			origin: { kind: 'joined' },
		};
		const plan = planReorder(tabs, [pair], [1, 2], measure, 360, true);
		expect(plan.order.map((t) => t.id)).toEqual([3, 4, 1, 2]);
		expect(plan.to).toBe(2);
	});

	it('holds a grouped tab in its group until its centre passes the group’s span', () => {
		const grouped = [tab(1, { group: 1 }), tab(2, { group: 1 }), tab(3), tab(4)];
		const inside = planReorder(grouped, [], [1], { spans, chips: [] }, 190, true);
		expect(inside.leaving).toBeNull();
		expect(inside.to).toBe(1);
		const outside = planReorder(grouped, [], [1], { spans, chips: [] }, 330, true);
		expect(outside.leaving).toBe(1);
		expect(outside.order.map((t) => t.id)).toEqual([2, 3, 1, 4]);
		// A group dragged by its chip never leaves itself.
		expect(planReorder(grouped, [], [1, 2], { spans, chips: [] }, 330, false).leaving).toBeNull();
	});

	it('counts the chip as part of the group’s span', () => {
		const grouped = [tab(1, { group: 1 }), tab(2), tab(3), tab(4)];
		const withChip = { spans, chips: [{ group: 1, left: -60, right: 0 }] };
		expect(planReorder(grouped, [], [1], withChip, -30, true).leaving).toBeNull();
		expect(planReorder(grouped, [], [1], withChip, -61, true).leaving).toBe(1);
	});
});

describe('slotLeft', () => {
	it('is where the unit’s first tab will sit, in the tablist’s coordinates', () => {
		const order = [tabs[1]!, tabs[2]!, tabs[0]!, tabs[3]!];
		const shifts = new Map([
			[2, -100],
			[3, -100],
		]);
		expect(slotLeft(tabs, spans, shifts, order, [1], 0)).toBe(200);
		expect(slotLeft(tabs, spans, shifts, order, [1], 40)).toBe(160);
		expect(slotLeft(tabs, spans, new Map(), [tabs[0]!, tabs[1]!], [1], 0)).toBe(100);
	});
});
