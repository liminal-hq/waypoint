// What a pair's menus, keys and panes ask of the session: split, separate, swap, resize, close and move
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { PairLayout } from '@liminal-hq/waypoint-protocol/generated/PairLayout';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { useMemo } from 'react';
import { showNotice } from '../app/notices';
import { t, tf } from '../i18n/messages';
import type { TabsApi } from '../services/tabsApi';
import { announce } from './announcer';
import { colourMessageId } from './tabColours';
import { equalSizes, paneAfter, pairOfTab } from './pairLayout';
import { requestPaneFocus } from './paneFocus';
import type { TabActions } from './tabActions';
import { useTabActions } from './tabActions';
import { useTabsApi, useTabsSnapshot } from './TabsContext';
import { useWindowActions, type MoveSpeech, type WindowActions } from './windowActions';
import { locationLabel } from './tabTitle';

export interface PairActions {
	/**
	 * F3: splits a tab into a pair with a copy of itself and focuses the new pane; on a pair the
	 * toggle made, closes that pane (with an Undo toast); on a hand-joined pair, only separates.
	 */
	toggleSplit(tab: TabId): void;
	/** Joins `tab` and `other` into a side-by-side pair (the menu path for hold-to-split). */
	splitWith(tab: TabId, other: TabId): void;
	separate(pair: Pair): void;
	swapPanes(pair: Pair): void;
	setLayout(pair: Pair, layout: PairLayout): void;
	resetSizes(pair: Pair): void;
	/** Writes the sizes (in thousandths) to the session; resolves when Rust has them. */
	setSizes(pair: Pair, sizes: number[]): Promise<void>;
	pin(pair: Pair, pinned: boolean): void;
	/** Colours every pane: Rust keeps a colour per tab, and a pair shows one colour. */
	setColour(pair: Pair, colour: TabColour | null): void;
	/** Opens a copy of each pane after the pair and joins the copies. */
	duplicate(pair: Pair): void;
	moveToNewWindow(pair: Pair): void;
	/** Hands the pair to the end of another window's strip. */
	moveToWindow(pair: Pair, target: WindowSummary): void;
	/** Closes one pane's tab; the pair separates and the other pane stays as a single tab. */
	closePane(tab: TabId): void;
	closeBoth(pair: Pair): void;
	/** F6: focuses the next (or, with a negative `delta`, previous) pane of the active pair. */
	focusPane(delta: 1 | -1): void;
}

function report(error: unknown): void {
	console.warn('pair command failed', error);
}

export function createPairActions(
	api: TabsApi,
	snapshot: SessionSnapshot | null,
	tabActions: Pick<TabActions, 'close' | 'activate'>,
	windows: Pick<WindowActions, 'moveMany'>,
): PairActions {
	const tabs = snapshot?.tabs ?? [];
	const pairs = snapshot?.pairs ?? [];
	const run = (work: Promise<unknown>) => void work.catch(report);
	const title = (id: TabId): string => {
		const tab = tabs.find((candidate) => candidate.id === id);
		return tab ? locationLabel(tab.location) : '';
	};
	const sentence = (pair: Pair) => pair.panes.map(title);
	const pairSpeech = (pair: Pair): MoveSpeech => ({
		newWindow: tf('pair.announce.movedWindow', {
			titles: sentence(pair).join(` ${t('pair.and')} `),
		}),
		toWindow: (window) =>
			tf('pair.announce.movedToWindow', {
				titles: sentence(pair).join(` ${t('pair.and')} `),
				window,
			}),
	});

	/** Reopens the closed pane and joins it back with the pane it was split from. */
	const undoClose = async (before: Pair, created: TabId) => {
		const restored = await api.reopenTab(created);
		if (restored === null) return announce(t('pair.announce.undoFailed'));
		try {
			await api.joinPair(before.panes, before.layout);
			const rejoined = (await api.getSnapshot()).pairs.find((pair) => pair.panes.includes(created));
			if (rejoined) await api.setPairSizes(rejoined.id, before.sizes);
		} catch (error) {
			// The other pane was paired again in the meantime: the pane still comes back, on its own.
			report(error);
		}
		requestPaneFocus(created);
		announce(tf('pair.announce.restored', { title: title(created) }));
	};

	const closeCreatedPane = async (pair: Pair, created: TabId) => {
		// The store closes the pane the toggle made, records it in Recently Closed with its history
		// and leaves the other pane active when this one was.
		const paneTitle = title(created);
		await api.toggleSplit(created);
		const keep = pair.panes.find((pane) => pane !== created);
		if (keep !== undefined) requestPaneFocus(keep);
		showNotice(tf('pair.announce.closedPane', { title: paneTitle }), {
			label: t('notice.undo'),
			run: () => run(undoClose(pair, created)),
		});
	};

	return {
		toggleSplit: (tab) => {
			const pair = pairOfTab(pairs, tab);
			if (!pair) {
				run(
					api.toggleSplit(tab).then(async () => {
						const fresh = await api.getSnapshot();
						const made = pairOfTab(fresh.pairs, tab);
						const created = made?.origin.kind === 'toggle' ? made.origin.created : undefined;
						if (created === undefined) return;
						await api.activateTab(created);
						requestPaneFocus(created);
						announce(tf('pair.announce.split', { title: title(tab) }));
					}),
				);
				return;
			}
			if (pair.origin.kind === 'toggle' && pair.panes.includes(pair.origin.created)) {
				run(closeCreatedPane(pair, pair.origin.created));
				return;
			}
			run(
				api.separatePair(pair.id).then(() =>
					announce(
						tf('pair.announce.separated', {
							first: title(pair.panes[0]!),
							second: title(pair.panes[1]!),
						}),
					),
				),
			);
		},
		splitWith: (tab, other) =>
			run(
				api
					.joinPair([tab, other], 'sideBySide')
					.then(() =>
						announce(tf('pair.announce.joined', { first: title(tab), second: title(other) })),
					),
			),
		separate: (pair) =>
			run(
				api.separatePair(pair.id).then(() =>
					announce(
						tf('pair.announce.separated', {
							first: title(pair.panes[0]!),
							second: title(pair.panes[1]!),
						}),
					),
				),
			),
		swapPanes: (pair) =>
			run(api.swapPanes(pair.id).then(() => announce(t('pair.announce.swapped')))),
		setLayout: (pair, layout) =>
			run(
				api.setPairLayout(pair.id, layout).then(() =>
					announce(
						tf('pair.announce.layout', {
							layout: t(layout === 'stacked' ? 'pair.layout.stacked' : 'pair.layout.sideBySide'),
						}),
					),
				),
			),
		resetSizes: (pair) =>
			run(
				api
					.setPairSizes(pair.id, equalSizes(pair.panes.length))
					.then(() => announce(t('pair.announce.sizesReset'))),
			),
		setSizes: (pair, sizes) => api.setPairSizes(pair.id, sizes),
		pin: (pair, pinned) => {
			const lead = pair.panes[0];
			if (lead === undefined) return;
			// A pair is pinned as a whole, whichever pane is named.
			run(
				api.pinTab(lead, pinned).then(() =>
					announce(
						tf(pinned ? 'pair.announce.pinned' : 'pair.announce.unpinned', {
							titles: sentence(pair).join(` ${t('pair.and')} `),
						}),
					),
				),
			);
		},
		setColour: (pair, colour) =>
			run(
				Promise.all(pair.panes.map((pane) => api.setTabColour(pane, colour))).then(() =>
					announce(
						colour
							? tf('pair.announce.colour', {
									colour: t(colourMessageId(colour)),
									titles: sentence(pair).join(` ${t('pair.and')} `),
								})
							: tf('pair.announce.colourCleared', {
									titles: sentence(pair).join(` ${t('pair.and')} `),
								}),
					),
				),
			),
		duplicate: (pair) =>
			run(
				(async () => {
					let after = pair.panes[pair.panes.length - 1];
					const copies: TabId[] = [];
					for (const pane of pair.panes) {
						const source = tabs.find((candidate) => candidate.id === pane);
						if (!source || after === undefined) return;
						after = await api.openTab(source.location, { after, activate: false });
						copies.push(after);
					}
					await api.joinPair(copies, pair.layout);
					announce(
						tf('pair.announce.duplicated', { titles: sentence(pair).join(` ${t('pair.and')} `) }),
					);
				})(),
			),
		moveToNewWindow: (pair) =>
			void windows.moveMany({ kind: 'pair', value: pair.id }, pair.panes, null, pairSpeech(pair)),
		moveToWindow: (pair, target) =>
			void windows.moveMany({ kind: 'pair', value: pair.id }, pair.panes, target, pairSpeech(pair)),
		closePane: (tab) => {
			const pair = pairOfTab(pairs, tab);
			// D29 (ask first when the other pane is the target of a running operation) waits for the
			// operations milestone; until jobs exist there is nothing to ask about.
			tabActions.close(tab);
			if (pair) {
				const other = pair.panes.find((pane) => pane !== tab);
				if (other !== undefined) {
					announce(tf('pair.announce.closedPaneKept', { closed: title(tab), kept: title(other) }));
				}
			}
		},
		closeBoth: (pair) => {
			for (const pane of pair.panes) tabActions.close(pane);
			announce(t('pair.announce.closedBoth'));
		},
		focusPane: (delta) => {
			const active = snapshot?.active;
			const pair = pairOfTab(pairs, active);
			if (!pair || active == null) return;
			const next = paneAfter(pair, active, delta);
			if (next === undefined) return;
			requestPaneFocus(next);
			tabActions.activate(next);
			announce(
				tf('pair.announce.focused', {
					position: pair.panes.indexOf(next) + 1,
					count: pair.panes.length,
					title: title(next),
				}),
			);
		},
	};
}

export function usePairActions(): PairActions {
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const tabActions = useTabActions();
	const windows = useWindowActions();
	return useMemo(
		() => createPairActions(api, snapshot, tabActions, windows),
		[api, snapshot, tabActions, windows],
	);
}
