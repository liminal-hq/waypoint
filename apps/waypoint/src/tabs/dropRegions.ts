// The tab strip's drop regions for another window's tear-off, and what a hit on one means
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Region } from '../services/tearoffClient';
import type { Rect } from './dragLayout';

/** The strip as a whole: a release here appends to the end of the strip. */
export const STRIP_REGION = 'strip';

/** A place between tabs, named by the index a moved tab would take: `slot:0` is before the first tab. */
const SLOT_PREFIX = 'slot:';

/** Where a hit on a region puts the tabs, in the window that registered it. */
export type DropSlot = { kind: 'end' } | { kind: 'slot'; index: number };

export function slotRegionId(index: number): string {
	return `${SLOT_PREFIX}${index}`;
}

/** What a region id from `buildDropRegions` means, or null for an id that is not one of ours. */
export function parseRegionId(id: string): DropSlot | null {
	if (id === STRIP_REGION) return { kind: 'end' };
	if (!id.startsWith(SLOT_PREFIX)) return null;
	const digits = id.slice(SLOT_PREFIX.length);
	if (!/^\d+$/.test(digits)) return null;
	return { kind: 'slot', index: Number(digits) };
}

/** A tab on screen: its place in the session's order and its extent in the window's coordinates. */
export interface StripSlot {
	index: number;
	left: number;
	right: number;
}

/** A group's chip on screen: the tabs it heads (a collapsed group's are hidden) and its extent in the window's coordinates. */
export interface StripChip {
	/** The session index of the group's first tab. */
	firstIndex: number;
	/** How many tabs the group has, hidden or not. */
	count: number;
	collapsed: boolean;
	left: number;
	right: number;
}

/**
 * The regions a window registers: the whole strip (merge at the end), under a half of each visible
 * tab (merge next to that tab: the left half puts the tabs before it, the right half after it).
 * A group's chip is before its first tab, or, for a collapsed group, its left half is before the
 * whole group and its right half after it. Tabs are clipped to the strip, since a scrolled strip
 * hides part of them, and the plugin picks the smallest region under the cursor, so a tab's half
 * wins over the strip behind it. The cuts are the ones `rawSlotAt` makes (`landing.ts`), which the
 * strip uses for the line it shows, so the slot of a hit is the slot that was shown.
 */
export function buildDropRegions(
	strip: Rect,
	slots: readonly StripSlot[],
	chips: readonly StripChip[] = [],
): Region[] {
	const height = strip.bottom - strip.top;
	const width = strip.right - strip.left;
	if (width <= 0 || height <= 0) return [];
	const region = (id: string, left: number, right: number): Region[] =>
		right > left ? [{ id, x: left, y: strip.top, width: right - left, height }] : [];
	// The strip's gaps and padding belong to the neighbour whose centre is nearer the cut, as in `rawSlotAt`: a
	// region reaches over the gap after it, and the first one over the padding before it.
	const items = [...slots, ...chips].map((item) => ({ left: item.left, right: item.right }));
	const reachRight = (right: number): number => {
		const next = items.filter((item) => item.left >= right).map((item) => item.left);
		return Math.min(strip.right, next.length > 0 ? Math.min(...next) : right);
	};
	const first = items.length > 0 ? Math.min(...items.map((item) => item.left)) : strip.left;
	const reachLeft = (left: number): number => (left <= first ? strip.left : left);
	const regions = region(STRIP_REGION, strip.left, strip.right);
	for (const slot of slots) {
		const left = Math.max(reachLeft(slot.left), strip.left);
		const right = Math.min(slot.right, strip.right);
		if (right <= Math.max(slot.left, strip.left)) continue;
		const middle = Math.min(Math.max((slot.left + slot.right) / 2, left), right);
		regions.push(
			...region(slotRegionId(slot.index), left, middle),
			...region(slotRegionId(slot.index + 1), middle, Math.max(right, reachRight(slot.right))),
		);
	}
	for (const chip of chips) {
		const left = Math.max(reachLeft(chip.left), strip.left);
		const right = Math.min(chip.right, strip.right);
		if (right <= Math.max(chip.left, strip.left)) continue;
		const reach = Math.max(right, reachRight(chip.right));
		if (!chip.collapsed) {
			regions.push(...region(slotRegionId(chip.firstIndex), left, reach));
			continue;
		}
		const middle = Math.min(Math.max((chip.left + chip.right) / 2, left), right);
		regions.push(
			...region(slotRegionId(chip.firstIndex), left, middle),
			...region(slotRegionId(chip.firstIndex + chip.count), middle, reach),
		);
	}
	return regions;
}

/** True when two region lists are the same, so an unchanged layout is not sent again. */
export function sameRegions(a: readonly Region[], b: readonly Region[]): boolean {
	return (
		a.length === b.length &&
		a.every((region, at) => {
			const other = b[at];
			return (
				other !== undefined &&
				region.id === other.id &&
				region.x === other.x &&
				region.y === other.y &&
				region.width === other.width &&
				region.height === other.height
			);
		})
	);
}
