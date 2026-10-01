// D29: closing one half of a pair asks first when the other half's folder is the destination of a running operation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import { createContext, useContext } from 'react';
import { pairOfTab } from './pairLayout';

/** Resolves to whether the tab may be closed. */
export type CloseGuard = (snapshot: SessionSnapshot, tab: TabId) => Promise<boolean>;

/** The folders of the other halves of the pair `tab` belongs to; empty for a tab that is not in a pair. */
export function otherHalves(snapshot: SessionSnapshot, tab: TabId): Location[] {
	const pair = pairOfTab(snapshot.pairs, tab);
	if (!pair) return [];
	return pair.panes
		.filter((pane) => pane !== tab)
		.flatMap((pane) => snapshot.tabs.find((candidate) => candidate.id === pane)?.location ?? []);
}

/**
 * The guard over a queue: a tab that is half of a pair may be closed at once unless a job still
 * writing (`jobsTargeting` lists the unfinished ones) works on the other half's folder, in which
 * case `confirm` decides. A queue that cannot answer never blocks a close.
 */
export function createCloseGuard(
	jobsTargeting: (location: Location) => Promise<number[]>,
	confirm: () => Promise<boolean>,
): CloseGuard {
	return async (snapshot, tab) => {
		for (const location of otherHalves(snapshot, tab)) {
			const jobs = await jobsTargeting(location).catch(() => []);
			if (jobs.length > 0) return confirm();
		}
		return true;
	};
}

const allow: CloseGuard = () => Promise.resolve(true);

const CloseGuardContext = createContext<CloseGuard>(allow);

export const CloseGuardProvider = CloseGuardContext.Provider;

export function useCloseGuard(): CloseGuard {
	return useContext(CloseGuardContext);
}
