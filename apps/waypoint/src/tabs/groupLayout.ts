// Where groups sit in the tab strip: the chips and tabs in render order, and where a move lands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Group } from '@liminal-hq/waypoint-protocol/generated/Group';
import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { Span } from './reorder';

/** A group past this many tabs shows its count and an amber outline; it is a nudge, never a limit. */
export const GROUP_SOFT_LIMIT = 8;

export interface ChipItem {
	kind: 'chip';
	group: Group;
	/** Every member, including the ones a collapsed group hides. */
	members: TabSnapshot[];
	/** The index in the session's tab list of the group's first tab. */
	firstIndex: number;
	pinned: boolean;
	/** The position among the pinned items, or -1 when the group is not pinned. */
	pinIndex: number;
	/** The window's active tab is one of the members. */
	containsActive: boolean;
}

export interface TabItem {
	kind: 'tab';
	tab: TabSnapshot;
	/** The index in the session's tab list. */
	index: number;
	/** The position among the pinned items, or -1 when the tab is not pinned. */
	pinIndex: number;
	group: Group | null;
	/** First and last visible tab of its group, for the bracket under the group's tabs. */
	groupFirst: boolean;
	groupLast: boolean;
}

export type StripItem = ChipItem | TabItem;

export interface StripLayout {
	items: StripItem[];
	/** How many sticky slots the pinned items take: pinned tabs and pinned groups' chips. */
	pinSlots: number;
}

/**
 * The strip in render order. A group's chip comes first and its tabs follow, unless the group is
 * collapsed, which hides them. The session keeps pinned items first and each group's tabs
 * contiguous, so one pass over the tab list is enough. A tab whose group is not in the snapshot
 * is drawn as an ordinary tab.
 */
export function buildStrip(snapshot: SessionSnapshot | null): StripLayout {
	const tabs = snapshot?.tabs ?? [];
	const groups = new Map((snapshot?.groups ?? []).map((group) => [group.id, group]));
	const items: StripItem[] = [];
	let pinSlots = 0;
	tabs.forEach((tab, index) => {
		const group = tab.group === null ? null : (groups.get(tab.group) ?? null);
		const startsGroup = group !== null && tabs[index - 1]?.group !== tab.group;
		if (group && startsGroup) {
			const members = tabs.filter((candidate) => candidate.group === group.id);
			items.push({
				kind: 'chip',
				group,
				members,
				firstIndex: index,
				pinned: tab.pinned,
				pinIndex: tab.pinned ? pinSlots++ : -1,
				containsActive: members.some((member) => member.id === snapshot?.active),
			});
		}
		if (group?.collapsed) return;
		items.push({
			kind: 'tab',
			tab,
			index,
			pinIndex: tab.pinned ? pinSlots++ : -1,
			group,
			groupFirst: startsGroup,
			groupLast: group !== null && tabs[index + 1]?.group !== tab.group,
		});
	});
	return { items, pinSlots };
}

/** The stop (a chip or a tab) that holds the roving tab stop and keyboard focus, as a stable key. */
export function stopKey(item: StripItem): string {
	return item.kind === 'chip' ? `group:${item.group.id}` : `tab:${item.tab.id}`;
}

/** The tab ids of one group, in strip order. */
export function groupTabs(tabs: readonly TabSnapshot[], group: GroupId): TabSnapshot[] {
	return tabs.filter((tab) => tab.group === group);
}

/**
 * The group of the active tab when that group is collapsed, so the active tab is hidden: the
 * chip then stands in for it as the strip's tab stop.
 */
export function hiddenActiveGroup(snapshot: SessionSnapshot | null): GroupId | null {
	const active = snapshot?.tabs.find((tab) => tab.id === snapshot.active);
	if (!active || active.group === null) return null;
	return snapshot?.groups.find((group) => group.id === active.group)?.collapsed
		? active.group
		: null;
}

/**
 * The extent of every tab in the session's order, for working out where a drag lands. A hidden
 * tab (in a collapsed group) takes its chip's extent, so passing the chip passes the whole group.
 */
export function measureSpans(root: ParentNode, tabs: readonly TabSnapshot[]): Span[] {
	const rectOf = (selector: string): Span | null => {
		const element = root.querySelector<HTMLElement>(selector);
		if (!element) return null;
		const { left, right } = element.getBoundingClientRect();
		return { left, right };
	};
	return tabs.map((tab, index) => {
		const own = rectOf(`[data-slot][data-index="${index}"]`);
		if (own) return own;
		const chip = tab.group === null ? null : rectOf(`[data-chip="${tab.group}"]`);
		return chip ?? { left: 0, right: 0 };
	});
}

/** The session's own order rules, for a list of tabs: pinned first, each group gathered at its first tab. */
function settle(tabs: readonly TabSnapshot[]): TabSnapshot[] {
	const blocks: { group: GroupId | null; pinned: boolean; tabs: TabSnapshot[] }[] = [];
	for (const tab of tabs) {
		const block = tab.group === null ? undefined : blocks.find((b) => b.group === tab.group);
		if (block) block.tabs.push(tab);
		else blocks.push({ group: tab.group, pinned: tab.pinned, tabs: [tab] });
	}
	return [
		...blocks.filter((b) => b.pinned).flatMap((b) => b.tabs),
		...blocks.filter((b) => !b.pinned).flatMap((b) => b.tabs),
	];
}

/**
 * Where the session leaves `moving` (one tab, or a whole group's tabs, in order) when asked to
 * put its first tab at `index` among the other tabs: a grouped tab stays inside its own group's
 * run, and anything else dropped inside another group's run, or across the pinned boundary, is
 * settled to the nearest legal place. Pairs are not modelled here: a pair moves as one unit in the
 * session, and the pair UI supplies that.
 */
export function settledOrder(
	tabs: readonly TabSnapshot[],
	moving: readonly TabId[],
	index: number,
	within: GroupId | null,
): TabSnapshot[] {
	const wanted = new Set(moving);
	const block = tabs.filter((tab) => wanted.has(tab.id));
	const rest = tabs.filter((tab) => !wanted.has(tab.id));
	let at = Math.max(0, Math.min(index, rest.length));
	if (within !== null) {
		const positions = rest.flatMap((tab, i) => (tab.group === within ? [i] : []));
		const first = positions[0];
		const last = positions[positions.length - 1];
		if (first !== undefined && last !== undefined) at = Math.max(first, Math.min(last + 1, at));
	}
	return settle([...rest.slice(0, at), ...block, ...rest.slice(at)]);
}

/** The index tab `from` ends up at when dropped at `to`, after the session's rules have settled it. */
export function landingIndex(tabs: readonly TabSnapshot[], from: number, to: number): number {
	const tab = tabs[from];
	if (!tab) return from;
	const order = settledOrder(tabs, [tab.id], to, tab.group);
	return order.findIndex((candidate) => candidate.id === tab.id);
}

/**
 * Where one keyboard step (`delta` of -1 or 1) puts tab `from`, as an index after the move. A tab
 * outside any group steps over a whole neighbouring group (collapsed or not) in one go, and a
 * grouped tab stays within its group: leaving one is Remove from Group.
 */
export function stepTarget(tabs: readonly TabSnapshot[], from: number, delta: -1 | 1): number {
	const tab = tabs[from];
	const next = tabs[from + delta];
	if (!tab || !next) return from;
	let to = from + delta;
	if (tab.group === null && next.group !== null) {
		const run = tabs.flatMap((candidate, i) => (candidate.group === next.group ? [i] : []));
		const edge = delta > 0 ? Math.max(...run) : Math.min(...run);
		to = edge;
	}
	return landingIndex(tabs, from, to);
}

/**
 * Where moving a whole group one place along puts its first tab: past the next tab, or past the
 * whole of the next group. `null` when it is already at that end of the strip.
 */
export function groupStepTarget(
	tabs: readonly TabSnapshot[],
	group: GroupId,
	delta: -1 | 1,
): number | null {
	const members = groupTabs(tabs, group);
	const first = tabs.findIndex((tab) => tab.id === members[0]?.id);
	const last = first + members.length - 1;
	const neighbour = tabs[delta > 0 ? last + 1 : first - 1];
	if (!neighbour || members.length === 0) return null;
	let to = delta > 0 ? first + 1 : first - 1;
	if (neighbour.group !== null) {
		const run = tabs.flatMap((tab, i) => (tab.group === neighbour.group ? [i] : []));
		// Moving left, the group goes before the neighbouring group's first tab.
		to = delta > 0 ? first + run.length : Math.min(...run);
	}
	const order = settledOrder(
		tabs,
		members.map((member) => member.id),
		to,
		null,
	);
	const landed = order.findIndex((tab) => tab.id === members[0]?.id);
	return landed === first ? null : landed;
}
