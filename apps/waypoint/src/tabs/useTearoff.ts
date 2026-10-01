// The plugin's feature flags for this window, read once at start-up, and the strip's drop regions kept registered
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef, useState, type MutableRefObject } from 'react';
import {
	NO_TEAROFF,
	type Region,
	type TearoffClient,
	type TearoffFeatures,
} from '../services/tearoffClient';
import { buildDropRegions, sameRegions, type StripSlot } from './dropRegions';
import { createTearHandoff, type TearHandoffDeps } from './tearOffHandoff';

/** How long a burst of layout changes settles before the regions are sent. */
export const REGION_DELAY_MS = 100;

/**
 * What the tear-off plugin can do here. The first answer can take a second on Wayland, so it is
 * asked once when the window starts (the client keeps it), and every feature is off until it comes:
 * a drag that begins sooner takes the in-page path. `live` always holds the newest value, for code
 * that outlives a render (the drag's hook).
 */
export function useTearoffFeatures(client: TearoffClient | undefined): {
	features: TearoffFeatures;
	live: MutableRefObject<TearoffFeatures>;
} {
	const [features, setFeatures] = useState<TearoffFeatures>(NO_TEAROFF);
	const live = useRef(features);
	live.current = features;
	useEffect(() => {
		if (!client) return;
		let active = true;
		client.features().then(
			(found) => {
				if (!active) return;
				live.current = found;
				setFeatures(found);
			},
			(error: unknown) => console.warn('could not read the tear-off features', error),
		);
		return () => {
			active = false;
		};
	}, [client]);
	return { features, live };
}

/** The strip's regions as the page lays them out now: the strip, and the visible tabs on it. */
export function measureDropRegions(root: ParentNode = document): Region[] {
	const strip = root.querySelector<HTMLElement>('[data-strip]');
	if (!strip) return [];
	const slots: StripSlot[] = [];
	for (const slot of strip.querySelectorAll<HTMLElement>('[data-slot][data-index]')) {
		const index = Number(slot.dataset.index);
		if (!Number.isFinite(index)) continue;
		const { left, right } = slot.getBoundingClientRect();
		slots.push({ index, left, right });
	}
	const { left, top, right, bottom } = strip.getBoundingClientRect();
	return buildDropRegions({ left, top, right, bottom }, slots);
}

/**
 * Registers this window's drop regions with the plugin while hit-testing works, and keeps them in
 * step with the strip: a throttled update on a resize, a scroll, and whenever `layoutKey` changes
 * (the tabs, their groups and their collapsed state). A new set replaces the old in one call, so a
 * hit test between two sets never sees none; the regions are cleared only when hit-testing goes off
 * or the window goes. A strip that is not mounted yet is waited for, and listeners attach when it is.
 */
export function useDropRegions(
	client: TearoffClient | undefined,
	enabled: boolean,
	layoutKey: string,
): void {
	const sent = useRef<Region[]>([]);
	const push = useRef<(() => void) | null>(null);

	// Clears the plugin's set only for good: not on a layout change.
	useEffect(() => {
		if (!client || !enabled) return;
		return () => {
			sent.current = [];
			client.setDropRegions([]).catch(() => {});
		};
	}, [client, enabled]);

	// Attaches the strip's listeners once, and waits for the strip when it is not there yet.
	useEffect(() => {
		if (!client || !enabled) return;
		let active = true;
		let timer: ReturnType<typeof setTimeout> | undefined;
		const send = () => {
			timer = undefined;
			if (!active) return;
			const regions = measureDropRegions();
			if (sameRegions(regions, sent.current)) return;
			sent.current = regions;
			client
				.setDropRegions(regions)
				.catch((error: unknown) => console.warn('could not register the drop regions', error));
		};
		const schedule = () => {
			if (timer === undefined) timer = setTimeout(send, REGION_DELAY_MS);
		};
		push.current = send;
		send();
		window.addEventListener('resize', schedule);
		let strip: Element | null = null;
		let tablist: Element | null = null;
		let observer: ResizeObserver | null = null;
		let mutations: MutationObserver | null = null;
		let waiter: MutationObserver | null = null;
		const attach = () => {
			if (!strip) {
				strip = document.querySelector('[data-strip]');
				if (strip) {
					strip.addEventListener('scroll', schedule, { passive: true });
					if (typeof ResizeObserver !== 'undefined') {
						observer = new ResizeObserver(schedule);
						observer.observe(strip);
					}
				}
			}
			// A tab arriving, closing or sliding changes its slot, which the resize of the strip does not see.
			if (strip && !tablist) {
				tablist = strip.querySelector('[role="tablist"]');
				if (tablist && typeof MutationObserver !== 'undefined') {
					mutations = new MutationObserver(schedule);
					mutations.observe(tablist, {
						childList: true,
						subtree: true,
						attributes: true,
						attributeFilter: ['data-index', 'data-group', 'hidden'],
					});
				}
			}
			return strip !== null && tablist !== null;
		};
		if (!attach() && typeof MutationObserver !== 'undefined') {
			waiter = new MutationObserver(() => {
				const done = attach();
				schedule();
				if (done) {
					waiter?.disconnect();
					waiter = null;
				}
			});
			waiter.observe(document.body, { childList: true, subtree: true });
		}
		return () => {
			active = false;
			push.current = null;
			if (timer !== undefined) clearTimeout(timer);
			strip?.removeEventListener('scroll', schedule);
			window.removeEventListener('resize', schedule);
			observer?.disconnect();
			mutations?.disconnect();
			waiter?.disconnect();
		};
	}, [client, enabled]);

	// A change of layout sends the new set at once, replacing the old.
	useEffect(() => {
		push.current?.();
	}, [layoutKey]);
}

/**
 * Acts, in the window that holds the tabs of a compositor-moved drag, on how it ended (see
 * `createTearHandoff`), and announces a payload dropped on this window. Runs for the window's life.
 */
export function useTearHandoff(
	client: TearoffClient | undefined,
	deps: Omit<TearHandoffDeps, 'client'>,
): void {
	const { api, flush, announce } = deps;
	useEffect(() => {
		if (!client) return;
		return createTearHandoff({ client, api, flush, announce }).connect();
	}, [client, api, flush, announce]);
}
