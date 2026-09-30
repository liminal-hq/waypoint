// What the tab strip, the shortcuts and the file views ask of the session, in one place
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import { useMemo } from 'react';
import type { TabsApi } from '../services/tabsApi';
import { useHomeLocation, useTabsApi, useTabsSnapshot } from './TabsContext';

export interface TabActions {
	/** A new tab in the current folder, next to the active tab, and active. */
	newTab(): void;
	/** A new tab at Home, at the end of the strip. */
	newTabAtHome(): void;
	/** Opens `location` in a tab next to the active one, without leaving the current tab. */
	openInBackground(location: Location): void;
	activate(tab: TabId): void;
	/** Closes a tab. Closing the last one leaves a fresh tab at Home: a window always has a tab until session restore and window closing arrive (milestone 3). */
	close(tab: TabId): void;
	/** Moves a tab to `index` (its position after the move). */
	move(tab: TabId, index: number): void;
	/** The tab `delta` places from the active one, wrapping round the ends. */
	cycle(delta: number): void;
	/** The tab at 1-based `position`, if the strip has that many. */
	goTo(position: number): void;
}

function report(error: unknown): void {
	console.warn('tab command failed', error);
}

/** Builds the commands over `api` for the session state in `snapshot` and a `home` location. */
export function createTabActions(
	api: TabsApi,
	snapshot: SessionSnapshot | null,
	home: Location,
): TabActions {
	const tabs = snapshot?.tabs ?? [];
	const active = tabs.find((tab) => tab.id === snapshot?.active);
	const run = (work: Promise<unknown>) => void work.catch(report);
	const activate = (tab: TabId) => {
		if (tab !== snapshot?.active) run(api.activateTab(tab));
	};
	return {
		newTab: () => run(api.openTab(active?.location ?? home, active ? { after: active.id } : {})),
		newTabAtHome: () => run(api.openTab(home)),
		openInBackground: (location) =>
			run(api.openTab(location, { activate: false, ...(active ? { after: active.id } : {}) })),
		activate,
		close: (tab) => {
			if (tabs.length === 1 && tabs[0]?.id === tab) {
				// Replace rather than empty the window, so it never shows nothing.
				run(api.openTab(home).then(() => api.closeTab(tab)));
			} else {
				run(api.closeTab(tab));
			}
		},
		move: (tab, index) => run(api.moveTab(tab, index)),
		cycle: (delta) => {
			if (tabs.length < 2 || !active) return;
			const from = tabs.indexOf(active);
			const next = tabs[(from + delta + tabs.length) % tabs.length];
			if (next) activate(next.id);
		},
		goTo: (position) => {
			const target = tabs[position - 1];
			if (target) activate(target.id);
		},
	};
}

export function useTabActions(): TabActions {
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const home = useHomeLocation();
	return useMemo(() => createTabActions(api, snapshot, home), [api, snapshot, home]);
}
