// Feeds the strip's landing line from the drags of other windows, and measures the strip it is drawn on
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { useEffect, useRef } from 'react';
import type { DragHover, MergeHover, TearoffClient } from '../services/tearoffClient';
import {
	createMergeLanding,
	LANDING_DRAG_STALE_MS,
	type MergeLandingStore,
	type StripMeasure,
} from './mergeLanding';
import { measureSpans } from './groupLayout';
import type { Span } from './reorder';
import { parseTearPayload } from './tearOffPayload';

/** The strip as the page lays it out now, or null while there is none. */
export function measureStripLanding(
	snapshot: SessionSnapshot | null,
	root: ParentNode = document,
): StripMeasure | null {
	const scroller = root.querySelector<HTMLElement>('[data-strip]');
	if (!scroller) return null;
	const tablist = scroller.querySelector('[role="tablist"]');
	const chips = new Map<GroupId, Span>();
	for (const chip of scroller.querySelectorAll<HTMLElement>('[data-chip]')) {
		const group = Number(chip.dataset.chip);
		if (!Number.isFinite(group)) continue;
		const { left, right } = chip.getBoundingClientRect();
		chips.set(group, { left, right });
	}
	const tabs = snapshot?.tabs ?? [];
	const { left, right, top, bottom } = scroller.getBoundingClientRect();
	return {
		tabs,
		pairs: snapshot?.pairs ?? [],
		spans: measureSpans(scroller, tabs),
		chips,
		strip: { left, right, top, bottom },
		tablistLeft: tablist ? tablist.getBoundingClientRect().left : left,
	};
}

/** What the plugin's hover over this window means as a merge hover; null for a payload that is not a tab drag. */
export function hoverFromPlugin(hover: DragHover): MergeHover | null {
	const payload = parseTearPayload(hover.payload);
	if (!payload) return null;
	return {
		x: hover.x,
		y: hover.y,
		region: hover.region,
		count: payload.tabs.length,
		pinned: payload.pinned === true,
	};
}

/**
 * Shows, on this window's strip, where tabs dragged here from another window would land: from the
 * ghost's `merge-hover` (X11, Windows) and the plugin's `drag-hover` (a Wayland window drag), each
 * ending with its leave. A drop on this window and the window losing focus clear it as well, and
 * the controller drops a hover that is not refreshed.
 */
export function useMergeLanding(
	client: TearoffClient | undefined,
	store: MergeLandingStore,
	announce: (text: string) => void,
	snapshot: SessionSnapshot | null,
): void {
	const latest = useRef(snapshot);
	latest.current = snapshot;
	useEffect(() => {
		if (!client) return;
		const landing = createMergeLanding({
			store,
			announce,
			measure: () => measureStripLanding(latest.current),
		});
		const stops = [
			client.onMergeHover((hover) => landing.hover(hover)),
			client.onMergeLeave(() => landing.leave()),
			client.onDragHover((hover) => {
				const mapped = hoverFromPlugin(hover);
				if (mapped) landing.hover(mapped, LANDING_DRAG_STALE_MS);
			}),
			client.onDragLeave(() => landing.leave()),
			// The tabs arrived: the strip has them now.
			client.onPayloadDropped(() => landing.clear()),
		];
		window.addEventListener('blur', landing.clear);
		return () => {
			window.removeEventListener('blur', landing.clear);
			for (const stop of stops) stop();
			landing.clear();
		};
	}, [client, store, announce]);
}
