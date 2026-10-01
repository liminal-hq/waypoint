// Dragging a pane header's grip up to the tab strip: the pointer path for Separate Tabs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PairId } from '@liminal-hq/waypoint-protocol/generated/PairId';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import { useRef, type PointerEvent } from 'react';
import type { DragHandlers } from '../dnd/dragSession';
import { t, tf } from '../i18n/messages';
import type { TabsApi } from '../services/tabsApi';
import { announce } from './announcer';
import { distanceFromStrip, outsideWindow, type Rect } from './dragLayout';
import type { TabDragSource, TearOffHook } from './tabDrag';
import { useSeparateSession, useTearOffHook } from './TabDragContext';
import { useTabsApi, useTabsSnapshot } from './TabsContext';
import { locationLabel } from './tabTitle';

/** What was grabbed: one pane's header of a pair, and where the strip it is dragged to was. */
export interface SeparateSource {
	pair: PairId;
	/** The pane whose header was grabbed; tearing it off takes this tab alone. */
	tab: TabId;
	strip: Rect;
}

export type SeparateTarget = { outcome: 'separate' } | { outcome: 'newWindow' };

/** The drag of one pane as the tear-off hook sees it: that tab alone, with nothing of the strip measured. */
function paneSource(source: SeparateSource): TabDragSource {
	return {
		kind: 'tab',
		unit: [source.tab],
		lead: source.tab,
		group: null,
		extent: { left: 0, right: 0 },
		measure: {
			spans: [],
			chips: [],
			strip: source.strip,
			tablistLeft: 0,
			area: null,
		},
		signature: '',
	};
}

/**
 * Over the strip a release separates the pair, as Separate Tabs does. Out of the window is the
 * new-window phase (`tearOff.ts`) for that one pane: a release opens it in a window of its own, or
 * merges it into another window, while its partner stays a single tab here. Anywhere else in the
 * window a release does nothing.
 */
export function createSeparateHandlers(deps: {
	api: TabsApi;
	snapshot(): SessionSnapshot | null;
	tearOff?: TearOffHook;
	viewport?(): { width: number; height: number };
}): DragHandlers<SeparateSource, SeparateTarget> {
	let out = false;
	return {
		move: (control, point) => {
			const away = distanceFromStrip(control.source.strip, point.y);
			const view = deps.viewport?.() ?? { width: window.innerWidth, height: window.innerHeight };
			if (outsideWindow(point, view)) {
				out = true;
				control.setTarget({ outcome: 'newWindow' });
				control.setPill(deps.tearOff?.update?.(point, paneSource(control.source)) ?? null);
				return;
			}
			if (out) {
				out = false;
				deps.tearOff?.leave?.();
			}
			if (away === 0) {
				control.setTarget({ outcome: 'separate' });
				control.setPill({
					kind: 'separate',
					text: t('drag.pill.separate'),
					announce: tf('drag.announce.pill', { text: t('drag.pill.separate').toLowerCase() }),
				});
			} else {
				control.setTarget(null);
				control.setPill(null);
			}
		},
		drop: async (control, point) => {
			if (control.target()?.outcome === 'newWindow') {
				await deps.tearOff?.drop?.(point, paneSource(control.source));
				return;
			}
			if (control.target()?.outcome !== 'separate') return;
			const snapshot = deps.snapshot();
			const pair = snapshot?.pairs.find((candidate) => candidate.id === control.source.pair);
			if (!snapshot || !pair) return;
			const title = (id: number) => {
				const tab = snapshot.tabs.find((candidate) => candidate.id === id);
				return tab ? locationLabel(tab.location) : '';
			};
			await deps.api.separatePair(pair.id);
			const titles = pair.panes.map(title);
			const last = titles[titles.length - 1] ?? '';
			announce(
				tf('pair.announce.separated', {
					titles:
						titles.length < 2 ? last : `${titles.slice(0, -1).join(', ')} ${t('pair.and')} ${last}`,
				}),
			);
		},
		cancel: (control) => {
			if (out) deps.tearOff?.leave?.();
			out = false;
			control.announce(t('drag.announce.cancelled'));
		},
	};
}

/** Starts the separate drag from a pane header's grip. */
export function useSeparateDrag(
	pair: PairId | undefined,
): (event: PointerEvent<HTMLElement>, tab: TabId) => void {
	const session = useSeparateSession();
	const tearOff = useTearOffHook();
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const latest = useRef(snapshot);
	latest.current = snapshot;
	return (event, tab) => {
		const strip = document.querySelector('[data-strip]');
		if (event.button !== 0 || pair === undefined || !strip) return;
		const { left, top, right, bottom } = strip.getBoundingClientRect();
		session.begin(
			{
				pointerId: event.pointerId,
				clientX: event.clientX,
				clientY: event.clientY,
				element: event.currentTarget,
				source: { pair, tab, strip: { left, top, right, bottom } },
			},
			createSeparateHandlers({ api, snapshot: () => latest.current, tearOff }),
		);
	};
}
