// Verifies the slot a foreign drop lands in, and where the line showing it goes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { describe, expect, it } from 'vitest';
import { buildDropRegions, parseRegionId, type StripChip, type StripSlot } from './dropRegions';
import { landedIndex, landingAt, rawSlotAt, rawSlotOf, type LandingInput } from './landing';
import type { Span } from './reorder';

function tab(id: number, group: number | null = null, pinned = false): TabSnapshot {
	return { id, pinned, group, hints: {} } as unknown as TabSnapshot;
}

const pair = (id: number, ...panes: number[]): Pair => ({ id, panes }) as unknown as Pair;

/** Tabs 100 wide from `left`, in a strip that starts at 0. */
function spansFor(count: number, left = 0): Span[] {
	return Array.from({ length: count }, (_, i) => ({
		left: left + i * 100,
		right: left + (i + 1) * 100,
	}));
}

function input(tabs: TabSnapshot[], extra: Partial<LandingInput> = {}): LandingInput {
	return {
		tabs,
		pairs: [],
		spans: spansFor(tabs.length),
		chips: new Map(),
		origin: 0,
		arriving: { count: 1, pinned: false },
		...extra,
	};
}

describe('rawSlotAt', () => {
	const spans = spansFor(3);
	it('is 0 in an empty strip', () => {
		expect(rawSlotAt(50, [])).toBe(0);
	});
	it('puts the left half of a tab before it and the right half after it', () => {
		expect(rawSlotAt(10, spans)).toBe(0);
		expect(rawSlotAt(49, spans)).toBe(0);
		expect(rawSlotAt(51, spans)).toBe(1);
		expect(rawSlotAt(149, spans)).toBe(1);
		expect(rawSlotAt(151, spans)).toBe(2);
	});
	it('is the ends past either end', () => {
		expect(rawSlotAt(-40, spans)).toBe(0);
		expect(rawSlotAt(900, spans)).toBe(3);
	});
	it('works on a scrolled strip, whose extents are measured where they are drawn', () => {
		// Scrolled 150 left: tab 0 spans -150..-50 and tab 1 -50..50.
		const scrolled = spansFor(4, -150);
		expect(rawSlotAt(-10, scrolled)).toBe(1);
		expect(rawSlotAt(10, scrolled)).toBe(2);
		expect(rawSlotAt(110, scrolled)).toBe(3);
		expect(rawSlotAt(-100, scrolled)).toBe(0);
	});
});

describe('rawSlotOf', () => {
	it('names the slot of a region and clamps to the strip', () => {
		expect(rawSlotOf({ kind: 'slot', index: 2 }, 5)).toBe(2);
		expect(rawSlotOf({ kind: 'slot', index: 9 }, 5)).toBe(5);
		expect(rawSlotOf({ kind: 'end' }, 5)).toBe(5);
	});
});

describe('landedIndex', () => {
	it('lands at the slot asked for in a plain strip', () => {
		const tabs = [tab(1), tab(2), tab(3)];
		for (let raw = 0; raw <= 3; raw++) {
			expect(landedIndex(tabs, [], raw, { count: 1, pinned: false })).toBe(raw);
		}
	});

	it('keeps an unpinned tab out of the pinned zone', () => {
		const tabs = [tab(1, null, true), tab(2, null, true), tab(3), tab(4)];
		expect(landedIndex(tabs, [], 0, { count: 1, pinned: false })).toBe(2);
		expect(landedIndex(tabs, [], 1, { count: 1, pinned: false })).toBe(2);
		expect(landedIndex(tabs, [], 2, { count: 1, pinned: false })).toBe(2);
		expect(landedIndex(tabs, [], 3, { count: 1, pinned: false })).toBe(3);
	});

	it('keeps a pinned tab in the pinned zone', () => {
		const tabs = [tab(1, null, true), tab(2, null, true), tab(3), tab(4)];
		expect(landedIndex(tabs, [], 4, { count: 1, pinned: true })).toBe(2);
		expect(landedIndex(tabs, [], 1, { count: 1, pinned: true })).toBe(1);
	});

	it('settles a slot inside a group to after the group', () => {
		const tabs = [tab(1), tab(2, 7), tab(3, 7), tab(4, 7), tab(5)];
		expect(landedIndex(tabs, [], 1, { count: 1, pinned: false })).toBe(1);
		expect(landedIndex(tabs, [], 2, { count: 1, pinned: false })).toBe(4);
		expect(landedIndex(tabs, [], 3, { count: 1, pinned: false })).toBe(4);
		expect(landedIndex(tabs, [], 4, { count: 1, pinned: false })).toBe(4);
	});

	it('treats a pair as one unit: a slot between its panes settles after the pair', () => {
		const tabs = [tab(1), tab(2), tab(3), tab(4)];
		const pairs = [pair(1, 2, 3)];
		expect(landedIndex(tabs, pairs, 1, { count: 1, pinned: false })).toBe(1);
		expect(landedIndex(tabs, pairs, 2, { count: 1, pinned: false })).toBe(3);
		expect(landedIndex(tabs, pairs, 3, { count: 1, pinned: false })).toBe(3);
	});

	it('keeps several arriving tabs together', () => {
		const tabs = [tab(1), tab(2), tab(3)];
		expect(landedIndex(tabs, [], 1, { count: 3, pinned: false })).toBe(1);
	});

	it('is 0 in an empty strip', () => {
		expect(landedIndex([], [], 0, { count: 2, pinned: false })).toBe(0);
	});
});

describe('landingAt', () => {
	it('puts the line on the edge between the tabs it lands between', () => {
		const tabs = [tab(1), tab(2), tab(3)];
		const at = (x: number) => landingAt(input(tabs), rawSlotAt(x, spansFor(3)));
		expect(at(20)).toMatchObject({ raw: 0, landed: 0, position: 1, edge: 0 });
		expect(at(80)).toMatchObject({ raw: 1, landed: 1, position: 2, edge: 100 });
		expect(at(120)).toMatchObject({ raw: 1, edge: 100 });
		expect(at(180)).toMatchObject({ raw: 2, edge: 200 });
		expect(at(290)).toMatchObject({ raw: 3, landed: 3, position: 4, edge: 300 });
	});

	it('shows the line at the start of an empty strip', () => {
		expect(landingAt(input([], { origin: 12 }), 0)).toMatchObject({
			raw: 0,
			landed: 0,
			edge: 12,
		});
	});

	it('shows the line where the store will put the tab, not where the pointer was', () => {
		const tabs = [tab(1, null, true), tab(2), tab(3)];
		// Over the left half of the first (pinned) tab, for an unpinned arrival.
		expect(landingAt(input(tabs), 0)).toMatchObject({ raw: 0, landed: 1, edge: 100 });
	});

	it('puts the line at a group edge, before its chip when the group starts the landing', () => {
		const tabs = [tab(1, 7), tab(2, 7), tab(3)];
		const chips = new Map([[7, { left: -40, right: 0 }]]);
		// Pretend the tabs sit after a 40-wide chip.
		const spans = spansFor(3, 0);
		const landing = landingAt(input(tabs, { spans, chips }), 0);
		expect(landing).toMatchObject({ landed: 0, edge: -40 });
		// After the group: the right edge of its last tab.
		expect(landingAt(input(tabs, { spans, chips }), 1)).toMatchObject({ landed: 2, edge: 200 });
	});

	it('uses the chip as the extent of a collapsed group', () => {
		// Tabs 2 and 3 are hidden in a collapsed group, so they take the chip's span.
		const tabs = [tab(1), tab(2, 7), tab(3, 7), tab(4)];
		const spans: Span[] = [
			{ left: 0, right: 100 },
			{ left: 100, right: 160 },
			{ left: 100, right: 160 },
			{ left: 160, right: 260 },
		];
		const chips = new Map([[7, { left: 100, right: 160 }]]);
		const model = input(tabs, { spans, chips });
		expect(landingAt(model, rawSlotAt(110, spans))).toMatchObject({ landed: 1, edge: 100 });
		expect(landingAt(model, rawSlotAt(150, spans))).toMatchObject({ landed: 3, edge: 160 });
	});

	it('lands a unit of several tabs once', () => {
		const model = input([tab(1), tab(2)], { arriving: { count: 3, pinned: false } });
		expect(landingAt(model, 1)).toMatchObject({ landed: 1, count: 3, edge: 100 });
	});
});

describe('the drop regions and the pointer agree', () => {
	/** The slot the plugin would report for a point: the smallest region under it. */
	function regionSlot(
		x: number,
		y: number,
		strip: { left: number; right: number },
		slots: StripSlot[],
		chips: StripChip[],
		tabCount: number,
	): number | null {
		const regions = buildDropRegions({ ...strip, top: 0, bottom: 30 }, slots, chips);
		let best: { area: number; id: string } | null = null;
		for (const region of regions) {
			if (x < region.x || x >= region.x + region.width || y < region.y) continue;
			if (y >= region.y + region.height) continue;
			const area = region.width * region.height;
			if (!best || area < best.area) best = { area, id: region.id };
		}
		const slot = best ? parseRegionId(best.id) : null;
		return slot ? rawSlotOf(slot, tabCount) : null;
	}

	it('names the same slot for every pointer position over a strip with groups', () => {
		// Tab 0, a chip and expanded group of tabs 1 and 2, a collapsed group of 3 and 4, then tab 5.
		const slots: StripSlot[] = [
			{ index: 0, left: 0, right: 100 },
			{ index: 1, left: 160, right: 260 },
			{ index: 2, left: 260, right: 360 },
			{ index: 5, left: 420, right: 520 },
		];
		const chips: StripChip[] = [
			{ firstIndex: 1, count: 2, collapsed: false, left: 100, right: 160 },
			{ firstIndex: 3, count: 2, collapsed: true, left: 360, right: 420 },
		];
		const spans: Span[] = [
			{ left: 0, right: 100 },
			{ left: 160, right: 260 },
			{ left: 260, right: 360 },
			{ left: 360, right: 420 },
			{ left: 360, right: 420 },
			{ left: 420, right: 520 },
		];
		for (let x = 0; x < 520; x++) {
			expect(regionSlot(x + 0.5, 10, { left: 0, right: 520 }, slots, chips, 6), `x=${x}`).toBe(
				rawSlotAt(x + 0.5, spans),
			);
		}
	});

	it('names the same slot on a scrolled strip, where the tabs are clipped to it', () => {
		const slots: StripSlot[] = [
			{ index: 0, left: -60, right: 40 },
			{ index: 1, left: 40, right: 140 },
			{ index: 2, left: 140, right: 240 },
		];
		const spans = slots.map(({ left, right }) => ({ left, right }));
		for (let x = 0; x < 200; x++) {
			expect(regionSlot(x + 0.5, 10, { left: 0, right: 200 }, slots, [], 3), `x=${x}`).toBe(
				rawSlotOf({ kind: 'slot', index: rawSlotAt(x + 0.5, spans) }, 3),
			);
		}
	});
});
