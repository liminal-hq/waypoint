// The frontend's view of the session plugin: tabs as a snapshot with a revision plus granular events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as session from '@liminal-hq/waypoint-plugin-session';
import type { Handoff } from '@liminal-hq/waypoint-plugin-session';
import type { Geometry } from '@liminal-hq/waypoint-protocol/generated/Geometry';
import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { GroupSort } from '@liminal-hq/waypoint-protocol/generated/GroupSort';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { MoveTo } from '@liminal-hq/waypoint-protocol/generated/MoveTo';
import type { MoveWhat } from '@liminal-hq/waypoint-protocol/generated/MoveWhat';
import type { PairId } from '@liminal-hq/waypoint-protocol/generated/PairId';
import type { PairLayout } from '@liminal-hq/waypoint-protocol/generated/PairLayout';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { TabHints } from '@liminal-hq/waypoint-protocol/generated/TabHints';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { ViewPrefs } from '@liminal-hq/waypoint-protocol/generated/ViewPrefs';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import type { WorkspaceId } from '@liminal-hq/waypoint-protocol/generated/WorkspaceId';
import type { Unsubscribe } from './vfsClient';

/** How many windows may be open at once; the plugin refuses a window past this (`MAX_WINDOWS` in Rust). */
export const MAX_WINDOWS = 12;

/** From this many windows on, opening one more tells the person that many are open (`WARN_WINDOWS` in Rust). */
export const WARN_WINDOWS = 8;

export type { Handoff };

/**
 * The window cap a rejected command names, or `null` when the rejection is anything else. The
 * plugin rejects with `{ kind: 'tooManyWindows', limit }` when a window would pass the cap.
 */
export function windowLimitOf(error: unknown): number | null {
	if (typeof error !== 'object' || error === null) return null;
	const { kind, limit } = error as { kind?: unknown; limit?: unknown };
	return kind === 'tooManyWindows' && typeof limit === 'number' ? limit : null;
}

export interface OpenTabOptions {
	/** Insert after this tab; the end of the strip when omitted. */
	after?: TabId;
	/** Make the new tab active (the first tab of a window always is). Defaults to true. */
	activate?: boolean;
}

/**
 * Everything the tab strip and the views need from the session plugin. Rust owns the tabs (A20):
 * read `getSnapshot()` once, then keep the copy current with `applyTabsEvent` on each event.
 * Commands resolve when Rust has applied them; the resulting change arrives as events.
 *
 * `tabsApi` wraps the plugin's guest-js; `FakeTabsApi` runs the same semantics in memory so the
 * UI can be built and tested without the Rust side.
 */
export interface TabsApi {
	getSnapshot(): Promise<SessionSnapshot>;
	openTab(location: Location, options?: OpenTabOptions): Promise<TabId>;
	closeTab(tab: TabId): Promise<void>;
	activateTab(tab: TabId): Promise<void>;
	moveTab(tab: TabId, index: number): Promise<void>;
	navigate(tab: TabId, location: Location): Promise<void>;
	back(tab: TabId): Promise<void>;
	forward(tab: TabId): Promise<void>;

	/** Pins or unpins a tab; a tab in a pair or group carries the whole pair or group with it. */
	pinTab(tab: TabId, pinned: boolean): Promise<void>;
	setTabColour(tab: TabId, colour: TabColour | null): Promise<void>;
	setTabHints(tab: TabId, hints: TabHints): Promise<void>;
	/** Reopens a closed tab (the newest when omitted); resolves to its id, or `null` when there was none. */
	reopenTab(tab?: TabId): Promise<TabId | null>;

	/** Groups the tabs (and the rest of their pairs); resolves to the new group's id. */
	createGroup(tabs: TabId[], name?: string): Promise<GroupId>;
	addToGroup(tab: TabId, group: GroupId): Promise<void>;
	removeFromGroup(tab: TabId): Promise<void>;
	renameGroup(group: GroupId, name: string): Promise<void>;
	setGroupColour(group: GroupId, colour: TabColour | null): Promise<void>;
	setGroupCollapsed(group: GroupId, collapsed: boolean): Promise<void>;
	/** Collapses every other group in this window. */
	collapseOtherGroups(group: GroupId): Promise<void>;
	sortGroup(group: GroupId, by: GroupSort): Promise<void>;
	/** Copies a group after itself; resolves to the copy's id. */
	duplicateGroup(group: GroupId): Promise<GroupId>;
	moveGroup(group: GroupId, index: number): Promise<void>;
	ungroup(group: GroupId): Promise<void>;
	closeGroup(group: GroupId): Promise<void>;

	/**
	 * Saves a group's folders as a workspace and resolves to its id. The name defaults to the
	 * group's; a name already in use rejects with a message that begins `a workspace named`
	 * (see `isWorkspaceNameTaken`).
	 */
	saveGroupAsWorkspace(group: GroupId, name?: string): Promise<WorkspaceId>;
	renameWorkspace(workspace: WorkspaceId, name: string): Promise<void>;
	deleteWorkspace(workspace: WorkspaceId): Promise<void>;
	/** Switches this window's Favourites to a workspace, or back to the bookmarks with `null`. */
	setActiveWorkspace(workspace: WorkspaceId | null): Promise<void>;
	/** Replaces a workspace's folders: add, remove and reorder are all this. */
	setWorkspaceLocations(workspace: WorkspaceId, locations: Location[]): Promise<void>;

	/** Pairs two or more tabs; resolves to the new pair's id. */
	joinPair(tabs: TabId[], layout: PairLayout): Promise<PairId>;
	separatePair(pair: PairId): Promise<void>;
	setPairLayout(pair: PairId, layout: PairLayout): Promise<void>;
	/** One share per pane, in thousandths adding up to 1000. */
	setPairSizes(pair: PairId, sizes: number[]): Promise<void>;
	swapPanes(pair: PairId): Promise<void>;
	/** Splits a tab into a pair with a copy of itself, or undoes such a split. */
	toggleSplit(tab: TabId): Promise<void>;

	/** Makes a window (with a first tab at `location` when given); resolves to its label. */
	openWindow(location?: Location, geometry?: Geometry): Promise<string>;
	/** Closes this window's session and the window, or the window named by `target`. */
	closeWindow(target?: string): Promise<void>;
	setGeometry(geometry: Geometry): Promise<void>;
	setView(view: ViewPrefs): Promise<void>;
	/**
	 * Moves tabs, a group or a pair out of this window into an existing window or a new one (do
	 * not name a label for a new window); resolves to the label of the window they went to.
	 */
	moveTabs(what: MoveWhat, to: MoveTo): Promise<string>;

	/** Every window of the session, this one marked `active`. */
	listWindows(): Promise<WindowSummary[]>;
	/** Follows the tabs other windows hand to this one. */
	onHandoff(listener: (handoff: Handoff) => void): Unsubscribe;

	/** Follows every change to this window's session. */
	onEvent(listener: (event: SessionEvent) => void): Unsubscribe;
}

/** Whether a rejection from `saveGroupAsWorkspace` or `renameWorkspace` means the name is in use. */
export function isWorkspaceNameTaken(error: unknown): boolean {
	return String(error).includes('a workspace named');
}

/**
 * Applies one event to a snapshot and returns the new snapshot. The store sends a window only its
 * own events but numbers them globally, so revisions have gaps; events at or below the
 * snapshot's revision are already included (a snapshot read after subscribing can overlap), so
 * they leave it unchanged. Closing a tab emits no event for the closed list, which is why
 * `closed` is only as fresh as the last snapshot.
 */
export function applyTabsEvent(snapshot: SessionSnapshot, event: SessionEvent): SessionSnapshot {
	if (event.revision <= snapshot.revision) return snapshot;
	const next: SessionSnapshot = { ...snapshot, revision: event.revision };
	switch (event.kind) {
		case 'tabOpened':
		case 'tabReopened':
			next.tabs = [...snapshot.tabs];
			next.tabs.splice(event.index, 0, event.tab);
			break;
		case 'tabClosed':
			next.tabs = snapshot.tabs.filter((t) => t.id !== event.tab);
			if (snapshot.active === event.tab) next.active = null;
			break;
		case 'tabActivated':
			next.active = event.tab;
			break;
		case 'tabMoved': {
			const from = snapshot.tabs.findIndex((t) => t.id === event.tab);
			if (from < 0) return snapshot;
			next.tabs = [...snapshot.tabs];
			next.tabs.splice(event.index, 0, ...next.tabs.splice(from, 1));
			break;
		}
		case 'tabNavigated':
		case 'tabChanged':
			next.tabs = snapshot.tabs.map((t) => (t.id === event.tab.id ? event.tab : t));
			break;
		case 'groupCreated':
			next.groups = [...snapshot.groups.filter((g) => g.id !== event.group.id), event.group];
			break;
		case 'groupChanged':
			next.groups = snapshot.groups.map((g) => (g.id === event.group.id ? event.group : g));
			break;
		case 'groupRemoved':
			next.groups = snapshot.groups.filter((g) => g.id !== event.group);
			break;
		case 'pairCreated':
			next.pairs = [...snapshot.pairs.filter((p) => p.id !== event.pair.id), event.pair];
			break;
		case 'pairChanged':
			next.pairs = snapshot.pairs.map((p) => (p.id === event.pair.id ? event.pair : p));
			break;
		case 'pairRemoved':
			next.pairs = snapshot.pairs.filter((p) => p.id !== event.pair);
			break;
		case 'mruChanged':
			next.mru = event.mru;
			break;
		case 'viewChanged':
			next.view = event.view;
			break;
		case 'geometryChanged':
			next.geometry = event.geometry;
			break;
		case 'workspacesChanged':
			next.workspaces = event.workspaces;
			break;
		case 'workspaceActivated':
			next.workspace = event.workspace;
			break;
		case 'windowOpened':
		case 'windowClosed':
			// Nothing in this window's own state changes; the revision still advances.
			break;
	}
	return next;
}

/**
 * The real implementation, over the plugin's guest-js. The plugin's listener registration is
 * asynchronous, so a listener added here starts receiving events once it resolves; events
 * missed in between are covered by reading the snapshot after subscribing.
 */
export const tabsApi: TabsApi = {
	getSnapshot: () => session.getSnapshot(),
	openTab: (location, options = {}) =>
		session.openTab(location, options.after, options.activate ?? true),
	closeTab: (tab) => session.closeTab(tab),
	activateTab: (tab) => session.activateTab(tab),
	moveTab: (tab, index) => session.moveTab(tab, index),
	navigate: (tab, location) => session.navigate(tab, location),
	back: (tab) => session.back(tab),
	forward: (tab) => session.forward(tab),
	pinTab: (tab, pinned) => session.pinTab(tab, pinned),
	setTabColour: (tab, colour) => session.setTabColour(tab, colour),
	setTabHints: (tab, hints) => session.setTabHints(tab, hints),
	reopenTab: (tab) => session.reopenTab(tab),
	createGroup: (tabs, name) => session.createGroup(tabs, name),
	addToGroup: (tab, group) => session.addToGroup(tab, group),
	removeFromGroup: (tab) => session.removeFromGroup(tab),
	renameGroup: (group, name) => session.renameGroup(group, name),
	setGroupColour: (group, colour) => session.setGroupColour(group, colour),
	setGroupCollapsed: (group, collapsed) => session.setGroupCollapsed(group, collapsed),
	collapseOtherGroups: (group) => session.collapseOtherGroups(group),
	sortGroup: (group, by) => session.sortGroup(group, by),
	duplicateGroup: (group) => session.duplicateGroup(group),
	moveGroup: (group, index) => session.moveGroup(group, index),
	ungroup: (group) => session.ungroup(group),
	closeGroup: (group) => session.closeGroup(group),
	saveGroupAsWorkspace: (group, name) => session.saveGroupAsWorkspace(group, name),
	renameWorkspace: (workspace, name) => session.renameWorkspace(workspace, name),
	deleteWorkspace: (workspace) => session.deleteWorkspace(workspace),
	setActiveWorkspace: (workspace) => session.setActiveWorkspace(workspace),
	setWorkspaceLocations: (workspace, locations) =>
		session.setWorkspaceLocations(workspace, locations),
	joinPair: (tabs, layout) => session.joinPair(tabs, layout),
	separatePair: (pair) => session.separatePair(pair),
	setPairLayout: (pair, layout) => session.setPairLayout(pair, layout),
	setPairSizes: (pair, sizes) => session.setPairSizes(pair, sizes),
	swapPanes: (pair) => session.swapPanes(pair),
	toggleSplit: (tab) => session.toggleSplit(tab),
	openWindow: (location, geometry) => session.openWindow(location, geometry),
	closeWindow: (target) => session.closeWindow(target),
	setGeometry: (geometry) => session.setGeometry(geometry),
	setView: (view) => session.setView(view),
	moveTabs: (what, to) => session.moveTabs(what, to),
	listWindows: () => session.listWindows(),
	onHandoff: (listener) => subscribe(session.onHandoff(listener)),
	onEvent: (listener) => subscribe(session.onTabsEvent(listener)),
};

/** Turns a listener registration that resolves later into an unsubscribe that works at once. */
function subscribe(registration: Promise<() => void>): Unsubscribe {
	let unlisten: (() => void) | undefined;
	let stopped = false;
	void registration.then((fn) => {
		if (stopped) fn();
		else unlisten = fn;
	});
	return () => {
		stopped = true;
		unlisten?.();
		unlisten = undefined;
	};
}
