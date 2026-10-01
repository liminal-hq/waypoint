// Exposes typed guest-side wrappers for the waypoint-session plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Geometry } from '@liminal-hq/waypoint-protocol/generated/Geometry';
import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { GroupSort } from '@liminal-hq/waypoint-protocol/generated/GroupSort';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { MoveTo } from '@liminal-hq/waypoint-protocol/generated/MoveTo';
import type { MoveWhat } from '@liminal-hq/waypoint-protocol/generated/MoveWhat';
import type { PairId } from '@liminal-hq/waypoint-protocol/generated/PairId';
import type { PairLayout } from '@liminal-hq/waypoint-protocol/generated/PairLayout';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { TabHints } from '@liminal-hq/waypoint-protocol/generated/TabHints';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { ViewPrefs } from '@liminal-hq/waypoint-protocol/generated/ViewPrefs';

const PREFIX = 'plugin:waypoint-session|';
const EVENT = 'waypoint-session://event';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports whether the session plugin works and which features it offers. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** The calling window's whole session at its current revision. */
export function getSnapshot(): Promise<SessionSnapshot> {
	return cmd<SessionSnapshot>('get_snapshot');
}

/** Opens a tab after `after` (or at the end) and returns its id. */
export function openTab(location: Location, after?: TabId, activate = true): Promise<TabId> {
	return cmd<TabId>('open_tab', { location, after: after ?? null, activate });
}

export function closeTab(tab: TabId): Promise<void> {
	return cmd<void>('close_tab', { tab });
}

export function activateTab(tab: TabId): Promise<void> {
	return cmd<void>('activate_tab', { tab });
}

export function moveTab(tab: TabId, index: number): Promise<void> {
	return cmd<void>('move_tab', { tab, index });
}

/** Goes somewhere new in a tab: the current location joins its back history. */
export function navigate(tab: TabId, location: Location): Promise<void> {
	return cmd<void>('navigate', { tab, location });
}

export function back(tab: TabId): Promise<void> {
	return cmd<void>('back', { tab });
}

export function forward(tab: TabId): Promise<void> {
	return cmd<void>('forward', { tab });
}

// Tabs.

/** Pins or unpins a tab; a tab in a pair or group carries the whole pair or group with it. */
export function pinTab(tab: TabId, pinned: boolean): Promise<void> {
	return cmd<void>('pin_tab', { tab, pinned });
}

export function setTabColour(tab: TabId, colour: TabColour | null): Promise<void> {
	return cmd<void>('set_tab_colour', { tab, colour });
}

export function setTabHints(tab: TabId, hints: TabHints): Promise<void> {
	return cmd<void>('set_tab_hints', { tab, hints });
}

/** Reopens a closed tab (the newest when `tab` is omitted) and returns its id, or `null` when there was none. */
export function reopenTab(tab?: TabId): Promise<TabId | null> {
	return cmd<TabId | null>('reopen_tab', { tab: tab ?? null });
}

// Groups.

/** Groups the tabs (and the rest of their pairs) and returns the new group's id. */
export function createGroup(tabs: TabId[], name?: string): Promise<GroupId> {
	return cmd<GroupId>('create_group', { tabs, name: name ?? null });
}

export function addToGroup(tab: TabId, group: GroupId): Promise<void> {
	return cmd<void>('add_to_group', { tab, group });
}

export function removeFromGroup(tab: TabId): Promise<void> {
	return cmd<void>('remove_from_group', { tab });
}

export function renameGroup(group: GroupId, name: string): Promise<void> {
	return cmd<void>('rename_group', { group, name });
}

export function setGroupColour(group: GroupId, colour: TabColour | null): Promise<void> {
	return cmd<void>('set_group_colour', { group, colour });
}

export function setGroupCollapsed(group: GroupId, collapsed: boolean): Promise<void> {
	return cmd<void>('set_group_collapsed', { group, collapsed });
}

/** Collapses every other group in the window. */
export function collapseOtherGroups(group: GroupId): Promise<void> {
	return cmd<void>('collapse_other_groups', { group });
}

export function sortGroup(group: GroupId, by: GroupSort): Promise<void> {
	return cmd<void>('sort_group', { group, by });
}

/** Copies a group after itself and returns the copy's id. */
export function duplicateGroup(group: GroupId): Promise<GroupId> {
	return cmd<GroupId>('duplicate_group', { group });
}

export function moveGroup(group: GroupId, index: number): Promise<void> {
	return cmd<void>('move_group', { group, index });
}

export function ungroup(group: GroupId): Promise<void> {
	return cmd<void>('ungroup', { group });
}

export function closeGroup(group: GroupId): Promise<void> {
	return cmd<void>('close_group', { group });
}

// Pairs.

/** Pairs two or more tabs and returns the new pair's id. */
export function joinPair(tabs: TabId[], layout: PairLayout): Promise<PairId> {
	return cmd<PairId>('join_pair', { tabs, layout });
}

export function separatePair(pair: PairId): Promise<void> {
	return cmd<void>('separate_pair', { pair });
}

export function setPairLayout(pair: PairId, layout: PairLayout): Promise<void> {
	return cmd<void>('set_pair_layout', { pair, layout });
}

/** Sets the share of each pane, in thousandths adding up to 1000. */
export function setPairSizes(pair: PairId, sizes: number[]): Promise<void> {
	return cmd<void>('set_pair_sizes', { pair, sizes });
}

export function swapPanes(pair: PairId): Promise<void> {
	return cmd<void>('swap_panes', { pair });
}

/** Splits a tab into a pair with a copy of itself, or undoes such a split. */
export function toggleSplit(tab: TabId): Promise<void> {
	return cmd<void>('toggle_split', { tab });
}

// Windows.

/** Makes a window, with a first tab at `location` when given, and returns its label. */
export function openWindow(location?: Location, geometry?: Geometry): Promise<string> {
	return cmd<string>('open_window', { location: location ?? null, geometry: geometry ?? null });
}

/** Closes the calling window's session and the window, or the window named by `target`. */
export function closeWindow(target?: string): Promise<void> {
	return cmd<void>('close_window', { target: target ?? null });
}

export function setGeometry(geometry: Geometry): Promise<void> {
	return cmd<void>('set_geometry', { geometry });
}

export function setView(view: ViewPrefs): Promise<void> {
	return cmd<void>('set_view', { view });
}

/** Moves tabs, a group or a pair out of the calling window and returns the label of the window they went to. */
export function moveTabs(what: MoveWhat, to: MoveTo): Promise<string> {
	return cmd<string>('move_tabs', { what, to });
}

/** Follows every change to the calling window's session. */
export function onTabsEvent(listener: (event: SessionEvent) => void): Promise<UnlistenFn> {
	return listen<SessionEvent>(EVENT, (e) => listener(e.payload));
}
