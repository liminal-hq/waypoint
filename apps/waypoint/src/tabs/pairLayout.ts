// Pure helpers for a pair's panes: who is visible, how a divider moves the sizes, and which pane is next
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';

/** The least a pane can shrink to, in thousandths of the pair's space. */
export const MIN_PANE_SHARE = 100;

/** How far one arrow key press moves a divider, in thousandths; Shift moves it further. */
export const DIVIDER_STEP = 20;
export const DIVIDER_BIG_STEP = 100;

/** Equal shares in thousandths adding up to 1000, the way Rust splits them (the first panes take the remainder). */
export function equalSizes(count: number): number[] {
	if (count <= 0) return [];
	const base = Math.floor(1000 / count);
	const extra = 1000 % count;
	return Array.from({ length: count }, (_, i) => base + (i < extra ? 1 : 0));
}

export function pairOfTab(pairs: readonly Pair[], tab: TabId | null | undefined): Pair | undefined {
	return tab == null ? undefined : pairs.find((pair) => pair.panes.includes(tab));
}

/** The pair the active tab is in, if any. */
export function activePair(snapshot: SessionSnapshot | null): Pair | undefined {
	return snapshot ? pairOfTab(snapshot.pairs, snapshot.active) : undefined;
}

/** The tabs on screen: the active tab, or every pane of the active tab's pair. */
export function visibleTabs(snapshot: SessionSnapshot | null): Set<TabId> {
	if (!snapshot || snapshot.active === null) return new Set();
	const pair = pairOfTab(snapshot.pairs, snapshot.active);
	return new Set(pair ? pair.panes : [snapshot.active]);
}

/**
 * Moves the divider after pane `index` by `delta` thousandths: the pane before it grows and the
 * one after it shrinks by the same amount, so the total never changes and neither goes under `min`.
 */
export function moveDivider(
	sizes: readonly number[],
	index: number,
	delta: number,
	min: number = MIN_PANE_SHARE,
): number[] {
	const next = [...sizes];
	const before = sizes[index];
	const after = sizes[index + 1];
	if (before === undefined || after === undefined) return next;
	const total = before + after;
	if (total < 2 * min) return next;
	const grown = Math.min(total - min, Math.max(min, Math.round(before + delta)));
	next[index] = grown;
	next[index + 1] = total - grown;
	return next;
}

export function sameSizes(a: readonly number[], b: readonly number[]): boolean {
	return a.length === b.length && a.every((value, i) => value === b[i]);
}

/** The pane `delta` places from `from`, wrapping round the ends. */
export function paneAfter(pair: Pair, from: TabId, delta: number): TabId | undefined {
	const at = pair.panes.indexOf(from);
	if (at < 0 || pair.panes.length < 2) return undefined;
	return pair.panes[(at + delta + pair.panes.length) % pair.panes.length];
}
