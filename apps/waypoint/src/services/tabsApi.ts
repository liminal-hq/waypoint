// The frontend's view of the session plugin: tabs as a snapshot with a revision plus granular events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as session from '@liminal-hq/waypoint-plugin-session';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { Unsubscribe } from './vfsClient';

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
	/** Follows every change to this window's session. */
	onEvent(listener: (event: SessionEvent) => void): Unsubscribe;
}

/**
 * Applies one event to a snapshot and returns the new snapshot. Events at or below the
 * snapshot's revision are already included (a snapshot read after subscribing can overlap), so
 * they leave it unchanged.
 */
export function applyTabsEvent(snapshot: SessionSnapshot, event: SessionEvent): SessionSnapshot {
	if (event.revision <= snapshot.revision) return snapshot;
	const next: SessionSnapshot = { ...snapshot, revision: event.revision };
	switch (event.kind) {
		case 'tabOpened':
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
			next.tabs = snapshot.tabs.map((t) => (t.id === event.tab.id ? event.tab : t));
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
	onEvent: (listener) => {
		let unlisten: (() => void) | undefined;
		let stopped = false;
		void session.onTabsEvent(listener).then((fn) => {
			if (stopped) fn();
			else unlisten = fn;
		});
		return () => {
			stopped = true;
			unlisten?.();
			unlisten = undefined;
		};
	},
};
