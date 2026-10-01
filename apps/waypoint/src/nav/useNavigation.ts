// What the active tab can do about navigation: the history it holds and the commands that move it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { useMemo } from 'react';
import { useVfsClient } from '../browse/VfsClientContext';
import { useActiveTab, useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { useLocationInfo } from './locationInfo';

export interface Navigation {
	tab: TabSnapshot | undefined;
	canGoBack: boolean;
	canGoForward: boolean;
	/** Up needs a parent, which Rust reports; false until it has answered. */
	canGoUp: boolean;
	goTo(location: Location): void;
	back(): void;
	forward(): void;
	up(): void;
	/** Steps back `steps` times: the history menu's jump to a folder further behind. */
	backBy(steps: number): void;
	forwardBy(steps: number): void;
}

function report(error: unknown): void {
	console.warn('navigation failed', error);
}

/**
 * With no `tabId` these follow the active tab; a pane passes its own tab's id.
 *
 * The session keeps each tab's history (A20); these commands only ask it to move. A step that
 * lands on a missing or unreadable folder is not refused here: the tab shows the error state and
 * its history stays intact, so Back still returns to where the person was.
 */
export function useNavigation(tabId?: number): Navigation {
	const api = useTabsApi();
	const client = useVfsClient();
	const active = useActiveTab();
	const snapshot = useTabsSnapshot();
	// A pane of a pair navigates its own tab, which may not be the active one yet.
	const tab =
		tabId === undefined ? active : snapshot?.tabs.find((candidate) => candidate.id === tabId);
	const info = useLocationInfo(client, tab?.location);
	const parent = info?.parent ?? null;
	const id = tab?.id;
	const backCount = tab?.back.length ?? 0;
	const forwardCount = tab?.forward.length ?? 0;

	return useMemo(() => {
		const repeat = async (step: (tab: number) => Promise<void>, times: number) => {
			if (id === undefined) return;
			for (let i = 0; i < times; i++) await step(id);
		};
		return {
			tab,
			canGoBack: backCount > 0,
			canGoForward: forwardCount > 0,
			canGoUp: parent !== null,
			goTo: (location) => {
				if (id !== undefined) api.navigate(id, location).catch(report);
			},
			back: () => void repeat((t) => api.back(t), 1).catch(report),
			forward: () => void repeat((t) => api.forward(t), 1).catch(report),
			up: () => {
				if (id !== undefined && parent) api.navigate(id, parent).catch(report);
			},
			backBy: (steps) => void repeat((t) => api.back(t), steps).catch(report),
			forwardBy: (steps) => void repeat((t) => api.forward(t), steps).catch(report),
		};
		// `tab` changes with every navigation, which is exactly when the counts do.
	}, [api, id, tab, backCount, forwardCount, parent]);
}
