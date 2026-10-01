// What dragging a tab means: reorder, split, group, leave a group, split a pane, and the new-window seam
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { DragControl, DragHandlers, DragPill, Point } from '../dnd/dragSession';
import { t, tf, tn, type MessageId } from '../i18n/messages';
import type { TabsApi } from '../services/tabsApi';
import {
	bodyAt,
	distanceFromStrip,
	edgeAt,
	planReorder,
	previewShifts,
	slotLeft,
	unitExtent,
	unitIds,
	type Edge,
	type ReorderPlan,
	type StripMeasure,
} from './dragLayout';
import { HOLD_JITTER_PX, HOLD_SPLIT_MS, REST_GROUP_MS, TEAR_OFF_PX } from './dragTiming';
import { pairName } from './PairPill';
import { groupTabs } from './groupLayout';
import type { Span } from './reorder';
import { locationLabel } from './tabTitle';

/** Every way a tab drag can end. A new one must be listed in `NON_POINTER_PATHS` too. */
export type TabDragOutcome =
	| 'reorder'
	| 'holdSplit'
	| 'holdGroup'
	| 'overGroupLabel'
	| 'leaveGroup'
	| 'splitPane'
	| 'newWindow'
	| 'cancel';

/** What was grabbed: a tab (with its pair), or a group by its chip. */
export interface TabDragSource {
	kind: 'tab' | 'group';
	/** The tabs that travel together, in strip order. */
	unit: TabId[];
	/** The tab that was pressed, or a group's first tab. */
	lead: TabId;
	/** The group being moved, for a chip drag. */
	group: GroupId | null;
	/** The unit's extent when the drag began, chip included for a group. */
	extent: Span;
	measure: StripMeasure;
	/** Changes when the session's tabs, groups or pairs do, which ends a drag. */
	signature: string;
}

/** What the strip draws while the drag runs: who slides and where the accent line or bracket sits. */
export interface TabDragPreview {
	shifts: [TabId, number][];
	/** Where the unit's first tab will sit, in the tablist's coordinates. */
	slot: number;
	marker: { kind: 'line' | 'bracket'; left: number; width: number } | null;
}

export type TabDragTarget =
	| { outcome: 'reorder'; to: number; preview: TabDragPreview }
	| {
			outcome: 'holdSplit';
			other: TabId;
			/** The ring has finished drawing and the two tabs have bridged. */
			armed: boolean;
			/** The dragged tab is the pair's first pane. */
			before: boolean;
			to: number;
			/** How far the dragged tab slides to sit against `other` once armed. */
			attach: number;
			preview: TabDragPreview;
	  }
	| { outcome: 'holdGroup'; to: number; neighbour: TabId; preview: TabDragPreview }
	| { outcome: 'overGroupLabel'; group: GroupId }
	| { outcome: 'leaveGroup'; group: GroupId; to: number; preview: TabDragPreview }
	| { outcome: 'splitPane'; edge: Edge }
	| { outcome: 'newWindow' };

/**
 * The seam for the new-window phase (slice 10 of milestone 3). The engine raises it once the
 * pointer is more than `TEAR_OFF_PX` out of the strip and not over a file-area edge zone. Until
 * something implements it the phase shows no pill and a release does nothing.
 */
export interface TearOffHook {
	/** The pointer is out of the strip; returns the pill to show, or null for none. */
	update?(point: Point, source: TabDragSource): DragPill | null;
	/** The pointer came back over the strip. */
	leave?(): void;
	/** A release in the new-window phase; resolve true when the hook took the tabs. */
	drop?(point: Point, source: TabDragSource): boolean | Promise<boolean>;
}

export interface TabDragDeps {
	api: TabsApi;
	/** The newest session snapshot, read each time it is needed. */
	snapshot(): SessionSnapshot | null;
	/** The live region's feed for results. */
	announce(text: string): void;
	/** Opens a new group's name for editing. */
	requestRename(group: GroupId): void;
	tearOff?: TearOffHook;
}

/** The layout a drop on `edge` of the file area gives, and whether the dragged tab is the first pane. */
export function edgeLayout(edge: Edge): { layout: 'sideBySide' | 'stacked'; before: boolean } {
	return {
		layout: edge === 'left' || edge === 'right' ? 'sideBySide' : 'stacked',
		before: edge === 'left' || edge === 'top',
	};
}

/** A tab may join another when they share a pin, and the dragged one is ungrouped or in the same group. */
export function canJoin(dragged: TabSnapshot, other: TabSnapshot): boolean {
	return (
		dragged.id !== other.id &&
		dragged.pinned === other.pinned &&
		(dragged.group === null || dragged.group === other.group)
	);
}

/** A string that changes when the tabs' order, groups, pairs or pins do; a drag does not outlive that. */
export function sessionSignature(snapshot: SessionSnapshot | null): string {
	if (!snapshot) return '';
	return JSON.stringify([
		snapshot.tabs.map((tab) => [tab.id, tab.group, tab.pinned]),
		snapshot.groups.map((group) => [group.id, group.collapsed]),
		snapshot.pairs.map((pair) => pair.panes),
	]);
}

/** Describes a tab drag from a press on `tab`, or on `group`'s chip, against the strip as measured. */
export function describeTabDrag(
	snapshot: SessionSnapshot,
	measure: StripMeasure,
	press: { tab: TabId } | { group: GroupId },
): TabDragSource | null {
	if ('group' in press) {
		const unit = groupTabs(snapshot.tabs, press.group).map((tab) => tab.id);
		const lead = unit[0];
		if (lead === undefined) return null;
		return {
			kind: 'group',
			unit,
			lead,
			group: press.group,
			extent: unitExtent(snapshot.tabs, unit, measure, press.group),
			measure,
			signature: sessionSignature(snapshot),
		};
	}
	const unit = unitIds(snapshot.tabs, snapshot.pairs, press.tab);
	if (unit.length === 0) return null;
	return {
		kind: 'tab',
		unit,
		lead: press.tab,
		group: null,
		extent: unitExtent(snapshot.tabs, unit, measure),
		measure,
		signature: sessionSignature(snapshot),
	};
}

function lowerFirst(text: string): string {
	return text.charAt(0).toLowerCase() + text.slice(1);
}

function pill(kind: string, text: string): DragPill {
	return {
		kind,
		text,
		announce: tf('drag.announce.pill', { text: lowerFirst(text) }),
	};
}

const EDGE_WORDS = {
	left: 'drag.edge.left',
	right: 'drag.edge.right',
	top: 'drag.edge.top',
	bottom: 'drag.edge.bottom',
} as const satisfies Record<Edge, MessageId>;

/** The tab beside the unit's new place that a new group would start with, if it may. */
function groupNeighbour(
	order: readonly TabSnapshot[],
	unit: readonly TabId[],
	dragged: TabSnapshot,
): TabSnapshot | null {
	const first = Math.min(...unit.map((id) => order.findIndex((tab) => tab.id === id)));
	const fits = (tab: TabSnapshot | undefined) =>
		tab !== undefined && tab.group === null && tab.pinned === dragged.pinned;
	const before = order[first - 1];
	if (fits(before)) return before ?? null;
	const after = order[first + unit.length];
	return fits(after) ? (after ?? null) : null;
}

/** The distance between two points. */
function distance(a: Point, b: Point): number {
	return Math.hypot(a.x - b.x, a.y - b.y);
}

/**
 * The outcomes of a tab drag over `dragSession`. Every move works out, in this order: the new-window
 * phase (more than `TEAR_OFF_PX` out of the strip, unless the pointer is over a file-area edge
 * zone, which splits that pane); a group chip under the pointer (add to the group); another tab's
 * body under the pointer (the ring, then the split); and otherwise where the unit would land, which
 * is a reorder, a leave when a grouped tab is past its group's span, or a new group once it has
 * rested in one slot. Release commits whatever is showing; Esc, or the browser taking the pointer,
 * abandons it and nothing changes.
 */
export function createTabDragHandlers(
	deps: TabDragDeps,
): DragHandlers<TabDragSource, TabDragTarget> {
	// The state of the holds: the tab being held over, and the slot being rested in.
	let split: {
		other: TabId;
		armed: boolean;
		plan: ReorderPlan;
		shifts: Map<TabId, number>;
	} | null = null;
	let rest: { key: string; anchor: Point; neighbour: TabId; armed: boolean } | null = null;
	let current: { plan: ReorderPlan; shifts: Map<TabId, number> } | null = null;
	let out = false;

	const clearHolds = (control: DragControl<TabDragSource, TabDragTarget>) => {
		control.clearHold('split');
		control.clearHold('group');
		split = null;
		rest = null;
	};

	const preview = (
		source: TabDragSource,
		snapshot: SessionSnapshot,
		plan: ReorderPlan,
		shifts: Map<TabId, number>,
		extra?: { kind: 'bracket'; neighbour: TabId },
	): TabDragPreview => {
		const width = source.extent.right - source.extent.left;
		const left = slotLeft(
			snapshot.tabs,
			source.measure.spans,
			shifts,
			plan.order,
			source.unit,
			source.measure.tablistLeft,
		);
		let marker: TabDragPreview['marker'] = { kind: 'line', left, width: 0 };
		if (extra) {
			const at = snapshot.tabs.findIndex((tab) => tab.id === extra.neighbour);
			const span = source.measure.spans[at];
			if (span) {
				const neighbourLeft =
					span.left + (shifts.get(extra.neighbour) ?? 0) - source.measure.tablistLeft;
				const start = Math.min(left, neighbourLeft);
				const end = Math.max(left + width, neighbourLeft + (span.right - span.left));
				marker = { kind: 'bracket', left: start, width: end - start };
			}
		}
		return { shifts: [...shifts.entries()], slot: left, marker };
	};

	const reorderPill = (
		source: TabDragSource,
		snapshot: SessionSnapshot,
		plan: ReorderPlan,
	): DragPill => {
		if (source.kind === 'group') {
			const group = snapshot.groups.find((candidate) => candidate.id === source.group);
			return pill('move', tf('drag.pill.moveGroup', { name: group?.name ?? '' }));
		}
		const lead = snapshot.tabs.find((tab) => tab.id === source.lead);
		const pair = snapshot.pairs.find((candidate) => candidate.panes.includes(source.lead));
		return pill(
			'move',
			tf('drag.pill.move', {
				title: pair ? pairName(pair, snapshot) : lead ? locationLabel(lead.location) : '',
				position: plan.to + 1,
				count: snapshot.tabs.length,
			}),
		);
	};

	/** Shows what a release would do now: the target and its pill. */
	const publish = (
		control: DragControl<TabDragSource, TabDragTarget>,
		snapshot: SessionSnapshot,
	) => {
		const source = control.source;
		if (!current) return;
		const { plan, shifts } = current;
		if (split) {
			const otherAt = snapshot.tabs.findIndex((tab) => tab.id === split?.other);
			const dragAt = snapshot.tabs.findIndex((tab) => tab.id === source.lead);
			const span = source.measure.spans[otherAt];
			const width = source.extent.right - source.extent.left;
			const before = dragAt < otherAt;
			const visualLeft = (span?.left ?? 0) + (split.shifts.get(split.other) ?? 0);
			const visualRight = (span?.right ?? 0) + (split.shifts.get(split.other) ?? 0);
			const attach = before
				? visualLeft - width + 2 - source.extent.left
				: visualRight - 2 - source.extent.left;
			const other = snapshot.tabs.find((tab) => tab.id === split?.other);
			control.setTarget({
				outcome: 'holdSplit',
				other: split.other,
				armed: split.armed,
				before,
				to: split.plan.to,
				attach,
				preview: preview(source, snapshot, split.plan, split.shifts),
			});
			control.setPill(
				split.armed && other
					? pill('split', tf('drag.pill.split', { name: locationLabel(other.location) }))
					: reorderPill(source, snapshot, split.plan),
			);
			return;
		}
		if (rest?.armed) {
			control.setTarget({
				outcome: 'holdGroup',
				to: plan.to,
				neighbour: rest.neighbour,
				preview: preview(source, snapshot, plan, shifts, {
					kind: 'bracket',
					neighbour: rest.neighbour,
				}),
			});
			control.setPill(pill('group', t('drag.pill.newGroup')));
			return;
		}
		if (plan.leaving !== null) {
			const group = snapshot.groups.find((candidate) => candidate.id === plan.leaving);
			control.setTarget({
				outcome: 'leaveGroup',
				group: plan.leaving,
				to: plan.to,
				preview: preview(source, snapshot, plan, shifts),
			});
			control.setPill(pill('leave', tf('drag.pill.leaveGroup', { name: group?.name ?? '' })));
			return;
		}
		control.setTarget({
			outcome: 'reorder',
			to: plan.to,
			preview: preview(source, snapshot, plan, shifts),
		});
		control.setPill(reorderPill(source, snapshot, plan));
	};

	const move = (control: DragControl<TabDragSource, TabDragTarget>, point: Point) => {
		const snapshot = deps.snapshot();
		if (!snapshot) return;
		const source = control.source;
		const measure = source.measure;
		const dragged = snapshot.tabs.find((tab) => tab.id === source.lead);
		if (!dragged) return;
		const tabs = snapshot.tabs;

		// Out of the strip: a file-area edge splits that pane, anything else is the new-window phase.
		if (distanceFromStrip(measure.strip, point.y) > TEAR_OFF_PX) {
			clearHolds(control);
			current = null;
			const edge = source.kind === 'tab' && measure.area ? edgeAt(measure.area, point) : null;
			const active = tabs.find((tab) => tab.id === snapshot.active);
			const paired = snapshot.pairs.some(
				(pair) => pair.panes.includes(source.lead) || (active && pair.panes.includes(active.id)),
			);
			if (edge && active && !paired && canJoin(dragged, active)) {
				if (out) deps.tearOff?.leave?.();
				out = false;
				control.setTarget({ outcome: 'splitPane', edge });
				control.setPill(pill('pane', tf('drag.pill.splitPane', { edge: t(EDGE_WORDS[edge]) })));
				return;
			}
			out = true;
			control.setTarget({ outcome: 'newWindow' });
			control.setPill(deps.tearOff?.update?.(point, source) ?? null);
			return;
		}
		if (out) {
			out = false;
			deps.tearOff?.leave?.();
		}

		// A group chip under the pointer takes the tab in.
		if (source.kind === 'tab') {
			const chip = measure.chips.find((c) => point.x >= c.left && point.x <= c.right);
			if (chip && dragged.group !== chip.group) {
				const group = snapshot.groups.find((candidate) => candidate.id === chip.group);
				if (group) {
					clearHolds(control);
					current = null;
					control.setTarget({ outcome: 'overGroupLabel', group: group.id });
					control.setPill(pill('add', tf('drag.pill.addToGroup', { name: group.name })));
					return;
				}
			}
		}

		const unitSet = new Set(source.unit);
		const width = source.extent.right - source.extent.left;
		const centre = (source.extent.left + source.extent.right) / 2 + (point.x - control.origin.x);
		const hiddenTabs = new Set(
			tabs
				.filter((tab) => snapshot.groups.some((g) => g.id === tab.group && g.collapsed))
				.map((tab) => tab.id),
		);
		const single = source.kind === 'tab' && source.unit.length === 1;

		// A tab already being held over keeps its place while the pointer stays on it.
		if (split) {
			const still = bodyAt(
				tabs,
				measure.spans,
				split.shifts,
				new Set([...unitSet, ...hiddenTabs]),
				point.x,
			);
			if (still === split.other) {
				current = { plan: split.plan, shifts: split.shifts };
				publish(control, snapshot);
				return;
			}
			control.clearHold('split');
			split = null;
		}

		const plan = planReorder(
			tabs,
			snapshot.pairs,
			source.unit,
			measure,
			centre,
			source.kind === 'tab',
		);
		const shifts = previewShifts(tabs, source.unit, plan.order, width);
		current = { plan, shifts };

		if (single && plan.leaving === null) {
			const candidate = bodyAt(
				tabs,
				measure.spans,
				shifts,
				new Set([...unitSet, ...hiddenTabs]),
				point.x,
			);
			const other = tabs.find((tab) => tab.id === candidate);
			const paired = (id: TabId) => snapshot.pairs.some((pair) => pair.panes.includes(id));
			if (other && canJoin(dragged, other) && !paired(other.id) && !paired(dragged.id)) {
				control.clearHold('group');
				rest = null;
				split = { other: other.id, armed: false, plan, shifts };
				control.hold('split', HOLD_SPLIT_MS, () => {
					if (!split) return;
					split.armed = true;
					const latest = deps.snapshot();
					if (latest) publish(control, latest);
				});
				publish(control, snapshot);
				return;
			}
		}

		// Resting in one slot, not hovering anything, starts a group.
		const neighbour =
			source.kind === 'tab' && dragged.group === null && plan.leaving === null
				? groupNeighbour(plan.order, source.unit, dragged)
				: null;
		if (!neighbour) {
			control.clearHold('group');
			rest = null;
		} else {
			const key = `${plan.to}:${neighbour.id}`;
			if (!rest || rest.key !== key || distance(point, rest.anchor) > HOLD_JITTER_PX) {
				rest = { key, anchor: point, neighbour: neighbour.id, armed: false };
				control.hold('group', REST_GROUP_MS, () => {
					if (!rest) return;
					rest.armed = true;
					const latest = deps.snapshot();
					if (latest) publish(control, latest);
				});
			}
		}
		publish(control, snapshot);
	};

	const commit = async (
		control: DragControl<TabDragSource, TabDragTarget>,
		point: Point,
	): Promise<void> => {
		const target = control.target();
		const snapshot = deps.snapshot();
		const source = control.source;
		if (!target || !snapshot) return;
		const { api } = deps;
		const tabs = snapshot.tabs;
		const lead = tabs.find((tab) => tab.id === source.lead);
		const title = (id: TabId) => {
			const tab = tabs.find((candidate) => candidate.id === id);
			return tab ? locationLabel(tab.location) : '';
		};
		const firstIndex = Math.min(...source.unit.map((id) => tabs.findIndex((tab) => tab.id === id)));
		switch (target.outcome) {
			case 'reorder': {
				if (target.to === firstIndex) return;
				if (source.kind === 'group' && source.group !== null) {
					const group = snapshot.groups.find((candidate) => candidate.id === source.group);
					await api.moveGroup(source.group, target.to);
					deps.announce(
						tf('groups.announce.moved', {
							name: group?.name ?? '',
							position: target.to + 1,
							count: tabs.length,
						}),
					);
					return;
				}
				await api.moveTab(source.lead, target.to);
				deps.announce(
					tf('tabs.moved', {
						title: title(source.lead),
						position: target.to + 1,
						count: tabs.length,
					}),
				);
				return;
			}
			case 'holdSplit': {
				// The held-over tab leads, so it stays where it is and the dragged tab comes to it.
				const pair = await api.joinPair([target.other, source.lead], 'sideBySide');
				if (target.before) await api.swapPanes(pair);
				const [first, second] = target.before
					? [source.lead, target.other]
					: [target.other, source.lead];
				deps.announce(tf('pair.announce.joined', { first: title(first), second: title(second) }));
				return;
			}
			case 'holdGroup': {
				if (target.to !== firstIndex) await api.moveTab(source.lead, target.to);
				const id = await api.createGroup([target.neighbour, source.lead]);
				const created = (await api.getSnapshot()).groups.find((group) => group.id === id);
				deps.announce(tf('groups.announce.created', { name: created?.name ?? '' }));
				// The new group opens with its name ready to edit.
				deps.requestRename(id);
				return;
			}
			case 'overGroupLabel': {
				const group = snapshot.groups.find((candidate) => candidate.id === target.group);
				if (!group || !lead) return;
				await api.addToGroup(source.lead, group.id);
				deps.announce(
					tf('groups.announce.added', {
						title: title(source.lead),
						name: group.name,
						tabs: tn('groups.tabCount', groupTabs(tabs, group.id).length + source.unit.length),
					}),
				);
				return;
			}
			case 'leaveGroup': {
				const group = snapshot.groups.find((candidate) => candidate.id === target.group);
				// The store keeps a grouped tab inside its group's run, so it leaves first and moves after.
				await api.removeFromGroup(source.lead);
				if (target.to !== firstIndex) await api.moveTab(source.lead, target.to);
				deps.announce(
					tf('groups.announce.removed', { title: title(source.lead), name: group?.name ?? '' }),
				);
				return;
			}
			case 'splitPane': {
				const active = snapshot.active;
				if (active === null) return;
				const { layout, before } = edgeLayout(target.edge);
				// The pane on show leads, so it keeps its place and group; the dragged tab joins it.
				const pair = await api.joinPair([active, source.lead], layout);
				if (before) await api.swapPanes(pair);
				const [first, second] = before ? [source.lead, active] : [active, source.lead];
				deps.announce(tf('pair.announce.joined', { first: title(first), second: title(second) }));
				return;
			}
			case 'newWindow':
				await deps.tearOff?.drop?.(point, source);
				return;
		}
	};

	return {
		move,
		drop: commit,
		cancel: (control) => {
			clearHolds(control);
			if (out) deps.tearOff?.leave?.();
			out = false;
			control.announce(t('drag.announce.cancelled'));
		},
	};
}

/** What each outcome is for someone who is not dragging: the menu item or key that does the same. */
export type NonPointerPath =
	{ kind: 'menu'; item: string } | { kind: 'keys'; keys: string } | { kind: 'none'; why: string };

/**
 * The non-pointer route to every outcome (docs/accessibility.md items 4 and 9). The type makes a
 * new outcome fail to compile until it has one, and the parity test checks each menu item exists.
 */
export const NON_POINTER_PATHS: Record<TabDragOutcome, NonPointerPath> = {
	reorder: { kind: 'keys', keys: 'Ctrl+Shift+ArrowLeft' },
	holdSplit: { kind: 'menu', item: 'splitWith' },
	holdGroup: { kind: 'menu', item: 'newGroup' },
	overGroupLabel: { kind: 'menu', item: 'addToGroup' },
	leaveGroup: { kind: 'menu', item: 'removeFromGroup' },
	splitPane: { kind: 'menu', item: 'splitWith' },
	newWindow: { kind: 'menu', item: 'moveToNewWindow' },
	cancel: {
		kind: 'none',
		why: 'nothing is dragged without a pointer, so there is nothing to cancel',
	},
};
