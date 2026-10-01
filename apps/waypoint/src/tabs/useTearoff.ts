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
 * (the tabs, their groups and their collapsed state). They are cleared when the window goes.
 */
export function useDropRegions(
	client: TearoffClient | undefined,
	enabled: boolean,
	layoutKey: string,
): void {
	useEffect(() => {
		if (!client || !enabled) return;
		let sent: Region[] = [];
		let timer: ReturnType<typeof setTimeout> | undefined;
		let active = true;
		const send = () => {
			timer = undefined;
			if (!active) return;
			const regions = measureDropRegions();
			if (sameRegions(regions, sent)) return;
			sent = regions;
			client
				.setDropRegions(regions)
				.catch((error: unknown) => console.warn('could not register the drop regions', error));
		};
		const schedule = () => {
			if (timer === undefined) timer = setTimeout(send, REGION_DELAY_MS);
		};
		send();
		const strip = document.querySelector('[data-strip]');
		strip?.addEventListener('scroll', schedule, { passive: true });
		window.addEventListener('resize', schedule);
		const observer =
			strip && typeof ResizeObserver !== 'undefined' ? new ResizeObserver(schedule) : null;
		if (strip) observer?.observe(strip);
		// A tab arriving, closing or sliding changes its slot, which the resize of the strip does not see.
		const tablist = strip?.querySelector('[role="tablist"]');
		const mutations =
			tablist && typeof MutationObserver !== 'undefined' ? new MutationObserver(schedule) : null;
		if (tablist)
			mutations?.observe(tablist, {
				childList: true,
				subtree: true,
				attributes: true,
				attributeFilter: ['data-index', 'data-group', 'hidden'],
			});
		return () => {
			active = false;
			if (timer !== undefined) clearTimeout(timer);
			strip?.removeEventListener('scroll', schedule);
			window.removeEventListener('resize', schedule);
			observer?.disconnect();
			mutations?.disconnect();
			client.setDropRegions([]).catch(() => {});
		};
	}, [client, enabled, layoutKey]);
}
