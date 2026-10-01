// Dragging a pane header's grip up to the tab strip: the pointer path for Separate Tabs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PairId } from '@liminal-hq/waypoint-protocol/generated/PairId';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { useRef, type PointerEvent } from 'react';
import type { DragHandlers } from '../dnd/dragSession';
import { t, tf } from '../i18n/messages';
import type { TabsApi } from '../services/tabsApi';
import { announce } from './announcer';
import { distanceFromStrip, type Rect } from './dragLayout';
import { useSeparateSession } from './TabDragContext';
import { useTabsApi, useTabsSnapshot } from './TabsContext';
import { locationLabel } from './tabTitle';

/** What was grabbed: a pair's pane header, and where the strip it is dragged to was. */
export interface SeparateSource {
	pair: PairId;
	strip: Rect;
}

export interface SeparateTarget {
	outcome: 'separate';
}

/**
 * Over the strip a release separates the pair, as Separate Tabs does; anywhere else a release does
 * nothing. Tearing one half off into a window is the new-window phase (slice 10 of milestone 3),
 * which a later change adds here the way `tabDrag.ts` has it.
 */
export function createSeparateHandlers(deps: {
	api: TabsApi;
	snapshot(): SessionSnapshot | null;
}): DragHandlers<SeparateSource, SeparateTarget> {
	return {
		move: (control, point) => {
			if (distanceFromStrip(control.source.strip, point.y) === 0) {
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
		drop: async (control) => {
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
		cancel: (control) => control.announce(t('drag.announce.cancelled')),
	};
}

/** Starts the separate drag from a pane header's grip. */
export function useSeparateDrag(
	pair: PairId | undefined,
): (event: PointerEvent<HTMLElement>) => void {
	const session = useSeparateSession();
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const latest = useRef(snapshot);
	latest.current = snapshot;
	return (event) => {
		const strip = document.querySelector('[data-strip]');
		if (event.button !== 0 || pair === undefined || !strip) return;
		const { left, top, right, bottom } = strip.getBoundingClientRect();
		session.begin(
			{
				pointerId: event.pointerId,
				clientX: event.clientX,
				clientY: event.clientY,
				element: event.currentTarget,
				source: { pair, strip: { left, top, right, bottom } },
			},
			createSeparateHandlers({ api, snapshot: () => latest.current }),
		);
	};
}
