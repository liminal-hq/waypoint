// What the tab strip's group chips and menus ask of the session, with the announcements that go with them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Group } from '@liminal-hq/waypoint-protocol/generated/Group';
import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { GroupSort } from '@liminal-hq/waypoint-protocol/generated/GroupSort';
import type { MoveWhat } from '@liminal-hq/waypoint-protocol/generated/MoveWhat';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { t, tf, tn } from '../i18n/messages';
import type { TabsApi } from '../services/tabsApi';
import { announce } from './announcer';
import { GROUP_SOFT_LIMIT, groupStepTarget, groupTabs } from './groupLayout';
import { colourMessageId } from './tabColours';
import { locationLabel } from './tabTitle';
import { useTabsApi, useTabsSnapshot } from './TabsContext';
import { useWindowActions, type MoveSpeech, type WindowActions } from './windowActions';

export interface GroupActions {
	/** Puts `tab` in a new group, named "Group N", and opens its name for editing. */
	newGroup(tab: TabSnapshot): void;
	addTo(tab: TabSnapshot, group: Group): void;
	removeFrom(tab: TabSnapshot): void;
	/** Renames; an empty name keeps the old one. */
	rename(group: Group, name: string): void;
	setColour(group: Group, colour: TabColour | null): void;
	setCollapsed(group: Group, collapsed: boolean): void;
	collapseOthers(group: Group): void;
	/** A new tab at the group's last tab's folder, inside the group. */
	newTabIn(group: Group): void;
	setPinned(group: Group, pinned: boolean): void;
	sort(group: Group, by: GroupSort): void;
	duplicate(group: Group): void;
	/** Hands the group to a window of its own. */
	moveToNewWindow(group: Group): void;
	/** Hands the group to the end of another window's strip. */
	moveToWindow(group: Group, target: WindowSummary): void;
	ungroup(group: Group): void;
	close(group: Group): void;
	/** The keyboard counterpart of dragging the chip: one place along the strip. */
	moveBy(group: Group, delta: -1 | 1): void;
}

const SORT_MESSAGES = {
	name: 'groups.menu.sortName',
	location: 'groups.menu.sortLocation',
	localFirst: 'groups.menu.sortLocal',
} as const;

function report(error: unknown): void {
	console.warn('group command failed', error);
}

function tabCount(count: number): string {
	return tn('groups.tabCount', count);
}

// The group whose name is open for editing is one piece of state shared by the chip (which edits
// it) and the menus that create groups, so it lives outside any one component.
let renaming: GroupId | null = null;
const renameListeners = new Set<() => void>();

function setRenaming(group: GroupId | null): void {
	renaming = group;
	renameListeners.forEach((listener) => listener());
}

/** Opens `group`'s name for editing on its chip. */
export function requestRename(group: GroupId): void {
	setRenaming(group);
}

/** Closes the name field without changing the name. */
export function endRename(): void {
	setRenaming(null);
}

/** The group being renamed in place, if any. */
export function useRenaming(): GroupId | null {
	return useSyncExternalStore(
		(listener) => {
			renameListeners.add(listener);
			return () => renameListeners.delete(listener);
		},
		() => renaming,
	);
}

/** Builds the group commands over `api` for the session state in `snapshot`; `windows` moves groups. */
export function createGroupActions(
	api: TabsApi,
	snapshot: SessionSnapshot | null,
	windows: Pick<WindowActions, 'moveMany'>,
): GroupActions {
	const tabs = snapshot?.tabs ?? [];
	const run = (work: Promise<unknown>) => void work.catch(report);
	const sizeOf = (group: Group) => groupTabs(tabs, group.id).length;
	const memberIds = (group: Group) => groupTabs(tabs, group.id).map((tab) => tab.id);
	const groupMove = (group: Group): MoveWhat => ({ kind: 'group', value: group.id });
	const groupSpeech = (group: Group): MoveSpeech => ({
		newWindow: tf('groups.announce.movedWindow', { name: group.name }),
		toWindow: (window) => tf('groups.announce.movedToWindow', { name: group.name, window }),
	});
	return {
		newGroup: (tab) =>
			run(
				api.createGroup([tab.id]).then(async (id) => {
					const created = (await api.getSnapshot()).groups.find((group) => group.id === id);
					announce(tf('groups.announce.created', { name: created?.name ?? '' }));
					requestRename(id);
				}),
			),
		addTo: (tab, group) =>
			run(
				api.addToGroup(tab.id, group.id).then(() =>
					announce(
						tf('groups.announce.added', {
							title: locationLabel(tab.location),
							name: group.name,
							tabs: tabCount(sizeOf(group) + (tab.group === group.id ? 0 : 1)),
						}),
					),
				),
			),
		removeFrom: (tab) => {
			const group = snapshot?.groups.find((candidate) => candidate.id === tab.group);
			run(
				api.removeFromGroup(tab.id).then(() =>
					announce(
						tf('groups.announce.removed', {
							title: locationLabel(tab.location),
							name: group?.name ?? '',
						}),
					),
				),
			);
		},
		rename: (group, name) => {
			const next = name.trim();
			if (next === '' || next === group.name) return;
			run(
				api
					.renameGroup(group.id, next)
					.then(() => announce(tf('groups.announce.renamed', { name: next }))),
			);
		},
		setColour: (group, colour) =>
			run(
				api.setGroupColour(group.id, colour).then(() =>
					announce(
						colour
							? tf('groups.announce.colour', {
									name: group.name,
									colour: t(colourMessageId(colour)),
								})
							: tf('groups.announce.colourCleared', { name: group.name }),
					),
				),
			),
		setCollapsed: (group, collapsed) =>
			run(
				api.setGroupCollapsed(group.id, collapsed).then(() =>
					announce(
						tf(collapsed ? 'groups.announce.collapsed' : 'groups.announce.expanded', {
							name: group.name,
							tabs: tabCount(sizeOf(group)),
						}),
					),
				),
			),
		collapseOthers: (group) =>
			run(
				api
					.collapseOtherGroups(group.id)
					.then(() => announce(t('groups.announce.collapsedOthers'))),
			),
		newTabIn: (group) => {
			const members = groupTabs(tabs, group.id);
			const last = members[members.length - 1];
			if (!last) return;
			run(
				api
					.openTab(last.location, { after: last.id })
					.then(() => announce(tf('groups.announce.newTab', { name: group.name }))),
			);
		},
		setPinned: (group, pinned) => {
			// Pinning any tab of a group pins the whole group.
			const first = groupTabs(tabs, group.id)[0];
			if (!first) return;
			run(
				api.pinTab(first.id, pinned).then(() =>
					announce(
						tf(pinned ? 'groups.announce.pinned' : 'groups.announce.unpinned', {
							name: group.name,
						}),
					),
				),
			);
		},
		sort: (group, by) =>
			run(
				api
					.sortGroup(group.id, by)
					.then(() =>
						announce(tf('groups.announce.sorted', { name: group.name, by: t(SORT_MESSAGES[by]) })),
					),
			),
		duplicate: (group) =>
			run(
				api
					.duplicateGroup(group.id)
					.then(() => announce(tf('groups.announce.duplicated', { name: group.name }))),
			),
		moveToNewWindow: (group) =>
			void windows.moveMany(groupMove(group), memberIds(group), null, groupSpeech(group)),
		moveToWindow: (group, target) =>
			void windows.moveMany(groupMove(group), memberIds(group), target, groupSpeech(group)),
		ungroup: (group) =>
			run(
				api
					.ungroup(group.id)
					.then(() => announce(tf('groups.announce.ungrouped', { name: group.name }))),
			),
		// Closing every tab of a window closes the window (D91): the session decides, and the tabs
		// go to Recently Closed.
		close: (group) =>
			run(
				api
					.closeGroup(group.id)
					.then(() => announce(tf('groups.announce.closed', { name: group.name }))),
			),
		moveBy: (group, delta) => {
			const target = groupStepTarget(tabs, group.id, delta, snapshot?.pairs);
			if (target === null) return;
			const position = target + 1;
			run(
				api
					.moveGroup(group.id, target)
					.then(() =>
						announce(
							tf('groups.announce.moved', { name: group.name, position, count: tabs.length }),
						),
					),
			);
		},
	};
}

export function useGroupActions(): GroupActions {
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const windows = useWindowActions();
	return useMemo(() => createGroupActions(api, snapshot, windows), [api, snapshot, windows]);
}

/**
 * Showing the active tab: when a tab inside a collapsed group becomes active (Ctrl+Tab, Alt+digit,
 * a closed neighbour, reopening) its group expands, since an active tab nobody can see is worse
 * than a group that is open. Collapsing the group that holds the active tab is still allowed, and
 * leaves it active but hidden until another tab takes over.
 */
export function useExpandOnActivate(): void {
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const lastActive = useRef<number | null | undefined>(undefined);
	useEffect(() => {
		const active = snapshot?.active ?? null;
		if (lastActive.current === active) return;
		const first = lastActive.current === undefined;
		lastActive.current = active;
		if (first || active === null || !snapshot) return;
		const tab = snapshot.tabs.find((candidate) => candidate.id === active);
		const group = snapshot.groups.find((candidate) => candidate.id === tab?.group);
		if (!tab || !group?.collapsed) return;
		void api
			.setGroupCollapsed(group.id, false)
			.then(() =>
				announce(
					tf('groups.announce.expandedForTab', {
						name: group.name,
						title: locationLabel(tab.location),
					}),
				),
			)
			.catch(report);
	}, [api, snapshot]);
}

/** A warning about a group that has grown past the soft limit. */
export interface GroupWarning {
	group: GroupId;
	text: string;
}

const WARNING_MS = 6000;

/**
 * Watches the session for a tab joining a group that is already over `GROUP_SOFT_LIMIT` (or taking
 * it past), and says so: aloud through the live region, and as a note the strip shows for a few
 * seconds. The limit is a nudge towards a split, so nothing is ever refused.
 */
export function useGroupLimitWarning(): GroupWarning | null {
	const snapshot = useTabsSnapshot();
	const sizes = useRef<Map<GroupId, number> | null>(null);
	const [warning, setWarning] = useState<GroupWarning | null>(null);
	useEffect(() => {
		if (!snapshot) return;
		const next = new Map<GroupId, number>();
		for (const tab of snapshot.tabs) {
			if (tab.group !== null) next.set(tab.group, (next.get(tab.group) ?? 0) + 1);
		}
		const previous = sizes.current;
		sizes.current = next;
		if (!previous) return;
		for (const [id, size] of next) {
			const before = previous.get(id);
			if (before === undefined || size <= before || size <= GROUP_SOFT_LIMIT) continue;
			const name = snapshot.groups.find((group) => group.id === id)?.name ?? '';
			const text = tf('groups.limit.warning', { name, count: size });
			announce(text);
			setWarning({ group: id, text });
			return;
		}
	}, [snapshot]);
	useEffect(() => {
		if (!warning) return;
		const timer = setTimeout(() => setWarning(null), WARNING_MS);
		return () => clearTimeout(timer);
	}, [warning]);
	return warning;
}
