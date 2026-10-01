// The target window's side of a tab merge: where tabs dragged here from another window would land, shown until they do or the drag moves on
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { createStore, type StoreApi } from 'zustand/vanilla';
import { tf } from '../i18n/messages';
import type { MergeHover } from '../services/tearoffClient';
import { parseRegionId } from './dropRegions';
import { landingAt, rawSlotAt, rawSlotOf, type Landing } from './landing';
import type { Span } from './reorder';

/** How long a hover stays when no newer one arrives: a drag that ended or moved away without saying so (a crashed window, a missed event). */
export const LANDING_STALE_MS = 400;

/**
 * How long a hover from the plugin's window drag stays without a newer one. The compositor's drag
 * sends motion only when the pointer moves, so a pointer held still sends nothing, and the plugin
 * guarantees a `drag-leave`; this is only the safety net for a leave that never came.
 */
export const LANDING_DRAG_STALE_MS = 15000;

/** What the strip draws: the line's place in the tablist's own coordinates, and what it stands for. */
export interface LandingView {
	left: number;
	position: number;
	count: number;
}

export interface MergeLandingState {
	view: LandingView | null;
}

export type MergeLandingStore = StoreApi<MergeLandingState>;

export function createMergeLandingStore(): MergeLandingStore {
	return createStore<MergeLandingState>(() => ({ view: null }));
}

/** The strip as the page lays it out now, in client coordinates. */
export interface StripMeasure {
	tabs: readonly TabSnapshot[];
	pairs: readonly Pair[];
	/** Every tab's extent in the session's order; a hidden tab takes its chip's. */
	spans: readonly Span[];
	chips: ReadonlyMap<GroupId, Span>;
	/** The strip's own box: the line shows only for a pointer over its height, unless the region says where. */
	strip: { left: number; right: number; top: number; bottom: number };
	/** The tablist's left edge, which the line is placed from. */
	tablistLeft: number;
}

export interface MergeLandingDeps {
	store: MergeLandingStore;
	/** The strip now, or null while there is none. */
	measure(): StripMeasure | null;
	/** The live region's feed. */
	announce(text: string): void;
	staleMs?: number;
	setTimer?(run: () => void, ms: number): unknown;
	clearTimer?(timer: unknown): void;
}

/**
 * Which slot a hover means. A region the plugin found is the slot a release merges at, so it is
 * used as it is. Without one, a pointer over the strip's own height is mapped from its x with the
 * same cut the regions make (`rawSlotAt`), and a pointer elsewhere over the window means the end,
 * where a merge with no slot appends.
 */
export function hoverSlot(hover: MergeHover, strip: StripMeasure): number {
	const region = hover.region === null ? null : parseRegionId(hover.region);
	if (region) return rawSlotOf(region, strip.tabs.length);
	const over =
		hover.y >= strip.strip.top &&
		hover.y < strip.strip.bottom &&
		hover.x >= strip.strip.left &&
		hover.x < strip.strip.right;
	return over ? rawSlotAt(hover.x, strip.spans) : strip.tabs.length;
}

/** What a hover shows, from the strip as measured. */
export function landingFor(hover: MergeHover, strip: StripMeasure): Landing {
	return landingAt(
		{
			tabs: strip.tabs,
			pairs: strip.pairs,
			spans: strip.spans,
			chips: strip.chips,
			origin: strip.strip.left,
			arriving: { count: Math.max(1, hover.count), pinned: hover.pinned },
		},
		hoverSlot(hover, strip),
	);
}

/**
 * Keeps the store showing where a drag from another window is over this strip. A hover shows the
 * line (a changed one moves it), and the first one of a visit says so once, politely, in the live
 * region. `leave` (the drag moved away, was dropped or ended) and a hover that has not been
 * refreshed for `staleMs` clear it, as does `clear`, which the window calls on blur.
 */
export function createMergeLanding(deps: MergeLandingDeps) {
	const staleMs = deps.staleMs ?? LANDING_STALE_MS;
	const setTimer = deps.setTimer ?? ((run: () => void, ms: number) => setTimeout(run, ms));
	const clearTimer =
		deps.clearTimer ?? ((timer: unknown) => clearTimeout(timer as ReturnType<typeof setTimeout>));
	let timer: unknown = null;

	const stopTimer = () => {
		if (timer !== null) clearTimer(timer);
		timer = null;
	};

	const clear = () => {
		stopTimer();
		if (deps.store.getState().view !== null) deps.store.setState({ view: null });
	};

	return {
		/** `staleOverride` replaces `staleMs` how long this hover stays without a refresh. */
		hover(hover: MergeHover, staleOverride?: number): void {
			const strip = deps.measure();
			if (!strip) return;
			const landing = landingFor(hover, strip);
			const view: LandingView = {
				left: landing.edge - strip.tablistLeft,
				position: landing.position,
				count: landing.count,
			};
			const previous = deps.store.getState().view;
			if (!previous) {
				deps.announce(
					tf(view.count > 1 ? 'drag.announce.landingMany' : 'drag.announce.landing', {
						position: String(view.position),
						count: String(view.count),
					}),
				);
			}
			if (
				!previous ||
				previous.left !== view.left ||
				previous.position !== view.position ||
				previous.count !== view.count
			) {
				deps.store.setState({ view });
			}
			stopTimer();
			timer = setTimer(clear, staleOverride ?? staleMs);
		},

		leave: clear,
		clear,
	};
}

export type MergeLanding = ReturnType<typeof createMergeLanding>;
