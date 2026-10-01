// The tab commands beyond open, close and move: pin, colour, duplicate, bulk close and reopen
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { useMemo } from 'react';
import { t, tf } from '../i18n/messages';
import type { TabsApi } from '../services/tabsApi';
import { announce } from './announcer';
import { colourMessageId } from './tabColours';
import type { TabActions } from './tabActions';
import { locationLabel } from './tabTitle';
import { useTabsApi } from './TabsContext';
import { useTabActions } from './tabActions';

export interface TabExtras {
	/** Pins or unpins `tab`. Rust moves it into or out of the pinned block. */
	pin(tab: TabSnapshot, pinned: boolean): void;
	setColour(tab: TabSnapshot, colour: TabColour | null): void;
	/** Opens another tab at the same location right after `tab`. The back and forward history is not copied. */
	duplicate(tab: TabSnapshot): void;
	/** Closes every unpinned tab but `tab`; pinned tabs are kept, as a bulk close should never lose them. */
	closeOthers(tab: TabSnapshot): void;
	/** Closes the unpinned tabs after `tab`. */
	closeToRight(tab: TabSnapshot): void;
	/** Reopens `closed`, or the most recently closed tab when omitted, and says so. */
	reopen(closed?: ClosedTab): void;
}

function report(error: unknown): void {
	console.warn('tab command failed', error);
}

export function createTabExtras(api: TabsApi, actions: Pick<TabActions, 'close'>): TabExtras {
	const run = (work: Promise<unknown>) => void work.catch(report);
	return {
		pin: (tab, pinned) =>
			run(
				api.pinTab(tab.id, pinned).then(() =>
					announce(
						tf(pinned ? 'tabs.announce.pinned' : 'tabs.announce.unpinned', {
							title: locationLabel(tab.location),
						}),
					),
				),
			),
		setColour: (tab, colour) =>
			run(
				api.setTabColour(tab.id, colour).then(() =>
					announce(
						colour
							? tf('tabs.announce.colour', {
									title: locationLabel(tab.location),
									colour: t(colourMessageId(colour)),
								})
							: tf('tabs.announce.colourCleared', { title: locationLabel(tab.location) }),
					),
				),
			),
		duplicate: (tab) =>
			run(
				api
					.openTab(tab.location, { after: tab.id })
					.then(() =>
						announce(tf('tabs.announce.duplicated', { title: locationLabel(tab.location) })),
					),
			),
		closeOthers: (tab) =>
			run(
				api.getSnapshot().then((latest) => {
					const doomed = latest.tabs.filter((other) => other.id !== tab.id && !other.pinned);
					for (const other of doomed) actions.close(other.id);
					announce(
						t(doomed.length ? 'tabs.announce.closedOthers' : 'tabs.announce.nothingToClose'),
					);
				}),
			),
		closeToRight: (tab) =>
			run(
				api.getSnapshot().then((latest) => {
					const from = latest.tabs.findIndex((candidate) => candidate.id === tab.id);
					const doomed =
						from < 0 ? [] : latest.tabs.slice(from + 1).filter((other) => !other.pinned);
					for (const other of doomed) actions.close(other.id);
					announce(t(doomed.length ? 'tabs.announce.closedRight' : 'tabs.announce.nothingToClose'));
				}),
			),
		reopen: (closed) =>
			run(
				api.reopenTab(closed?.tab.id).then(async (id) => {
					if (id === null) return announce(t('tabs.announce.noneClosed'));
					const reopened = (await api.getSnapshot()).tabs.find((tab) => tab.id === id);
					announce(
						tf('tabs.announce.reopened', {
							title: locationLabel((closed?.tab ?? reopened)?.location ?? { display: '' }),
						}),
					);
				}),
			),
	};
}

export function useTabExtras(): TabExtras {
	const api = useTabsApi();
	const actions = useTabActions();
	return useMemo(() => createTabExtras(api, actions), [api, actions]);
}
