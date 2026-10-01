// Where tabs dropped on this strip from another window would land: one computation for the line shown and the merge made
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { DropSlot } from './dropRegions';
import { settledOrder } from './groupLayout';
import type { Span } from './reorder';

/** What is arriving: how many tabs, and whether they sit in the pinned zone of the window they leave. */
export interface Arriving {
	count: number;
	pinned: boolean;
}

/**
 * The slot a pointer at `x` means: the number of tabs whose centre it is right of, so over a tab's
 * left half it is before that tab and over the right half after it, between tabs it is the gap, and
 * past either end it is that end. `spans` are the tabs' extents in the session's order and in the
 * same coordinates as `x`; a hidden tab (in a collapsed group) takes its chip's extent, so the
 * chip's own halves are before and after the whole group. The drop regions of the strip
 * (`buildDropRegions`) cut the strip at the same places.
 */
export function rawSlotAt(x: number, spans: readonly Span[]): number {
	let slot = 0;
	for (const span of spans) {
		if ((span.left + span.right) / 2 < x) slot++;
	}
	return slot;
}

/** The slot a region id names in a strip of `tabCount` tabs: the end is one past the last. */
export function rawSlotOf(slot: DropSlot, tabCount: number): number {
	return slot.kind === 'slot' ? Math.min(slot.index, tabCount) : tabCount;
}

/**
 * Where the session leaves arriving tabs that were asked to go in at `raw`: the pinned zone is kept
 * (arriving pinned tabs go after the last pinned one at the latest, others after it), and a slot
 * inside a group's run or between a pair's panes is settled to just after the group or the pair.
 * It is the store's own ordering, run on stand-ins for the arriving tabs, so the answer is what a
 * merge at `raw` produces. The result is the arriving tabs' first position among all the tabs after
 * the merge, which is also the number of the strip's present tabs before them.
 */
export function landedIndex(
	tabs: readonly TabSnapshot[],
	pairs: readonly Pair[],
	raw: number,
	arriving: Arriving,
): number {
	const count = Math.max(1, arriving.count);
	const template = tabs[0];
	// Ids no tab has (ids are never negative); only `pinned` and the absence of a group matter.
	const standIns = Array.from(
		{ length: count },
		(_, i) =>
			({
				...(template ?? {}),
				id: -1 - i,
				pinned: arriving.pinned,
				group: null,
			}) as unknown as TabSnapshot,
	);
	const all = [...tabs, ...standIns];
	const order = settledOrder(
		all,
		standIns.map((tab) => tab.id),
		Math.max(0, Math.min(raw, tabs.length)),
		null,
		pairs,
	);
	return Math.max(
		0,
		order.findIndex((tab) => tab.id === standIns[0]?.id),
	);
}

/**
 * The x of the line at `landed` (a slot from `landedIndex`): the right edge of the tab before it, or the
 * left edge of the one after when it is first, and a group's chip counts as part of its first tab
 * so the line is before the chip, not between the chip and the tab. 0 for an empty strip, where
 * `origin` is the strip's own left edge.
 */
export function landingEdge(
	tabs: readonly TabSnapshot[],
	spans: readonly Span[],
	chips: ReadonlyMap<GroupId, Span>,
	landed: number,
	origin: number,
): number {
	const before = spans[landed - 1];
	if (landed > 0 && before) return before.right;
	const first = tabs[landed];
	const next = spans[landed];
	if (first && next) {
		const chip = first.group === null ? undefined : chips.get(first.group);
		return chip ? Math.min(chip.left, next.left) : next.left;
	}
	return origin;
}

/** Everything the strip shows and a merge uses, for one hover. */
export interface Landing {
	/** The slot to ask the session for: `moveTabs`' `index`. */
	raw: number;
	/** Where the tabs end up among the strip's present tabs (they come after that many). */
	landed: number;
	/** 1-based, among all the tabs once they are in: what is announced. */
	position: number;
	/** Where the line goes, in the coordinates of `spans`. */
	edge: number;
	count: number;
}

export interface LandingInput {
	tabs: readonly TabSnapshot[];
	pairs: readonly Pair[];
	spans: readonly Span[];
	chips: ReadonlyMap<GroupId, Span>;
	/** The strip's left edge, for a strip with no tabs. */
	origin: number;
	arriving: Arriving;
}

/** What a hover at slot `raw` shows. The merge asks for the same `raw`, so it lands where the line was. */
export function landingAt(input: LandingInput, raw: number): Landing {
	const clamped = Math.max(0, Math.min(raw, input.tabs.length));
	const landed = landedIndex(input.tabs, input.pairs, clamped, input.arriving);
	return {
		raw: clamped,
		landed,
		position: landed + 1,
		edge: landingEdge(input.tabs, input.spans, input.chips, landed, input.origin),
		count: input.arriving.count,
	};
}
