// Geometry and previews for a tab drag: what was measured, where a release would land, who slides
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { Point } from '../dnd/dragSession';
import { EDGE_ZONE_FRACTION, TAB_BODY_FRACTION } from './dragTiming';
import { measureSpans, settledOrder } from './groupLayout';
import { clampToZone, unitDropIndex, type Span } from './reorder';

export interface Rect {
	left: number;
	top: number;
	right: number;
	bottom: number;
}

/** What the strip looked like when the drag began. A drag measures once and reuses it. */
export interface StripMeasure {
	/** Every tab's extent in the session's order; a hidden tab takes its group chip's. */
	spans: Span[];
	chips: { group: GroupId; left: number; right: number }[];
	strip: Rect;
	/** Where the tablist's own left edge was, to place markers inside it. */
	tablistLeft: number;
	/** The file area, for the edge zones; `null` when there is none on screen. */
	area: Rect | null;
}

export type Edge = 'left' | 'right' | 'top' | 'bottom';

function rectOf(element: Element): Rect {
	const { left, top, right, bottom } = element.getBoundingClientRect();
	return { left, top, right, bottom };
}

/** Reads the strip, its chips and the file area from the document. */
export function measureStrip(
	scroller: HTMLElement,
	tabs: readonly TabSnapshot[],
	root: ParentNode = document,
): StripMeasure {
	const area = root.querySelector('[data-pane-area]');
	const tablist = scroller.querySelector('[role="tablist"]');
	const chips = Array.from(scroller.querySelectorAll<HTMLElement>('[data-chip]')).flatMap(
		(chip) => {
			const group = Number(chip.dataset.chip);
			const { left, right } = chip.getBoundingClientRect();
			return Number.isFinite(group) ? [{ group, left, right }] : [];
		},
	);
	return {
		spans: measureSpans(scroller, tabs),
		chips,
		strip: rectOf(scroller),
		tablistLeft: tablist ? tablist.getBoundingClientRect().left : 0,
		area: area ? rectOf(area) : null,
	};
}

/** How far `y` is above or below the strip, or 0 inside it. */
export function distanceFromStrip(strip: Rect, y: number): number {
	return Math.max(strip.top - y, y - strip.bottom, 0);
}

/**
 * The edge of the file area the pointer is in the outer `fraction` of, or null in the middle or
 * outside the area. In a corner the nearer edge, by share of the area, wins.
 */
export function edgeAt(
	area: Rect,
	point: Point,
	fraction: number = EDGE_ZONE_FRACTION,
): Edge | null {
	const width = area.right - area.left;
	const height = area.bottom - area.top;
	if (width <= 0 || height <= 0) return null;
	const nx = (point.x - area.left) / width;
	const ny = (point.y - area.top) / height;
	if (nx < 0 || nx > 1 || ny < 0 || ny > 1) return null;
	const distances: [Edge, number][] = [
		['left', nx],
		['right', 1 - nx],
		['top', ny],
		['bottom', 1 - ny],
	];
	const nearest = distances.reduce((best, next) => (next[1] < best[1] ? next : best));
	return nearest[1] <= fraction ? nearest[0] : null;
}

/** The tabs that travel with `tab`: its pair's panes, or the tab alone; in strip order. */
export function unitIds(tabs: readonly TabSnapshot[], pairs: readonly Pair[], tab: TabId): TabId[] {
	const pair = pairs.find((candidate) => candidate.panes.includes(tab));
	return tabs.filter((t) => (pair ? pair.panes.includes(t.id) : t.id === tab)).map((t) => t.id);
}

/** Where a release would leave a unit, and what it would leave behind. */
export interface ReorderPlan {
	/** The index of the unit's first tab after the move, which is what `Move` takes. */
	to: number;
	/** The group the unit is being dragged out of, if its centre is past that group's span. */
	leaving: GroupId | null;
	/** The strip's order after the move. */
	order: TabSnapshot[];
}

/**
 * Works out where the unit lands when its centre is at `centre`. A grouped tab stays in its group
 * until its centre passes the group's span (the chip included), then leaves it: the plan is made
 * as if it were ungrouped, which is what Remove from Group followed by the move does. A single
 * tab stays on its own side of the pinned boundary.
 */
export function planReorder(
	tabs: readonly TabSnapshot[],
	pairs: readonly Pair[],
	unit: readonly TabId[],
	measure: Pick<StripMeasure, 'spans' | 'chips'>,
	centre: number,
	canLeave: boolean,
): ReorderPlan {
	const indices = unit.flatMap((id) => {
		const at = tabs.findIndex((tab) => tab.id === id);
		return at < 0 ? [] : [at];
	});
	const lead = tabs[indices[0] ?? 0];
	if (!lead) return { to: 0, leaving: null, order: [...tabs] };
	let working: readonly TabSnapshot[] = tabs;
	let leaving: GroupId | null = null;
	if (canLeave && lead.group !== null) {
		const members = tabs.flatMap((tab, i) => (tab.group === lead.group ? [i] : []));
		const first = measure.spans[members[0] ?? 0];
		const last = measure.spans[members[members.length - 1] ?? 0];
		const chip = measure.chips.find((candidate) => candidate.group === lead.group);
		const left = Math.min(first?.left ?? Infinity, chip?.left ?? Infinity);
		const right = last?.right ?? -Infinity;
		if (centre < left || centre > right) {
			leaving = lead.group;
			const moving = new Set(unit);
			working = tabs.map((tab) => (moving.has(tab.id) ? { ...tab, group: null } : tab));
		}
	}
	let to = unitDropIndex(measure.spans, indices, centre);
	if (indices.length === 1) {
		const pinnedCount = working.filter((tab) => tab.pinned).length;
		to = clampToZone(to, lead.pinned, pinnedCount, working.length);
	}
	const within = canLeave ? (working[indices[0] ?? 0]?.group ?? null) : null;
	const order = settledOrder(working, unit, to, within, pairs);
	const landed = Math.min(...unit.map((id) => order.findIndex((tab) => tab.id === id)));
	return { to: Number.isFinite(landed) && landed >= 0 ? landed : to, leaving, order };
}

/**
 * How far each tab other than the unit slides to open the unit's gap, keyed by tab id: a tab the
 * unit has passed moves into the space it left. `width` is the unit's extent, chip included.
 */
export function previewShifts(
	tabs: readonly TabSnapshot[],
	unit: readonly TabId[],
	order: readonly TabSnapshot[],
	width: number,
): Map<TabId, number> {
	const shifts = new Map<TabId, number>();
	const moving = new Set(unit);
	const oldAt = (id: TabId) => tabs.findIndex((tab) => tab.id === id);
	const newAt = (id: TabId) => order.findIndex((tab) => tab.id === id);
	const oldFirst = Math.min(...unit.map(oldAt));
	const oldLast = Math.max(...unit.map(oldAt));
	const newFirst = Math.min(...unit.map(newAt));
	for (const tab of tabs) {
		if (moving.has(tab.id)) continue;
		const before = oldAt(tab.id);
		const after = newAt(tab.id);
		if (before > oldLast && after < newFirst) shifts.set(tab.id, -width);
		else if (before < oldFirst && after > newFirst) shifts.set(tab.id, width);
	}
	return shifts;
}

/** The extent a unit takes in the strip: its tabs' spans, and its group's chip when the group moves. */
export function unitExtent(
	tabs: readonly TabSnapshot[],
	unit: readonly TabId[],
	measure: Pick<StripMeasure, 'spans' | 'chips'>,
	withChip: GroupId | null = null,
): Span {
	const spans = unit.flatMap((id) => {
		const span = measure.spans[tabs.findIndex((tab) => tab.id === id)];
		return span ? [span] : [];
	});
	const chip = withChip === null ? undefined : measure.chips.find((c) => c.group === withChip);
	const lefts = [...spans.map((s) => s.left), ...(chip ? [chip.left] : [])];
	const rights = [...spans.map((s) => s.right), ...(chip ? [chip.right] : [])];
	return { left: Math.min(...lefts), right: Math.max(...rights) };
}

/**
 * The tab whose body (the middle `fraction` of it) is under `x`, given how far each tab has
 * slid, or `null`. Tabs in `skip` (the dragged unit, hidden tabs) never match.
 */
export function bodyAt(
	tabs: readonly TabSnapshot[],
	spans: readonly Span[],
	shifts: ReadonlyMap<TabId, number>,
	skip: ReadonlySet<TabId>,
	x: number,
	fraction: number = TAB_BODY_FRACTION,
): TabId | null {
	for (let i = 0; i < tabs.length; i++) {
		const tab = tabs[i];
		const span = spans[i];
		if (!tab || !span || skip.has(tab.id) || span.right <= span.left) continue;
		const shift = shifts.get(tab.id) ?? 0;
		const inset = ((span.right - span.left) * (1 - fraction)) / 2;
		if (x >= span.left + shift + inset && x <= span.right + shift - inset) return tab.id;
	}
	return null;
}

/** The left edge, in the tablist's own coordinates, where the unit lands: the accent line's place. */
export function slotLeft(
	tabs: readonly TabSnapshot[],
	spans: readonly Span[],
	shifts: ReadonlyMap<TabId, number>,
	order: readonly TabSnapshot[],
	unit: readonly TabId[],
	tablistLeft: number,
): number {
	const first = Math.min(...unit.map((id) => order.findIndex((tab) => tab.id === id)));
	const spanOf = (tab: TabSnapshot | undefined) => {
		const at = tab ? tabs.findIndex((candidate) => candidate.id === tab.id) : -1;
		return at < 0 ? undefined : spans[at];
	};
	const before = order[first - 1];
	const beforeSpan = spanOf(before);
	if (before && beforeSpan) return beforeSpan.right + (shifts.get(before.id) ?? 0) - tablistLeft;
	const after = order[first + unit.length];
	const afterSpan = spanOf(after);
	if (after && afterSpan) return afterSpan.left + (shifts.get(after.id) ?? 0) - tablistLeft;
	return 0;
}
