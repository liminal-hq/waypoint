// Verifies the target window's landing line: what it shows for a hover, when it clears, and what is said
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { describe, expect, it } from 'vitest';
import { rawSlotAt } from './landing';
import type { MergeHover } from '../services/tearoffClient';
import {
	createMergeLanding,
	createMergeLandingStore,
	hoverSlot,
	LANDING_STALE_MS,
	landingFor,
	type StripMeasure,
} from './mergeLanding';

function tab(id: number, pinned = false): TabSnapshot {
	return { id, pinned, group: null, hints: {} } as unknown as TabSnapshot;
}

/** Three 100-wide tabs in a strip that starts at x = 20 and is 30 tall; the tablist starts at 20. */
function strip(tabs = [tab(1), tab(2), tab(3)]): StripMeasure {
	return {
		tabs,
		pairs: [],
		spans: tabs.map((_, i) => ({ left: 20 + i * 100, right: 120 + i * 100 })),
		chips: new Map(),
		strip: { left: 20, right: 520, top: 40, bottom: 70 },
		tablistLeft: 20,
	};
}

/** A hover at `x` over the strip: the plugin finds the region the strip's own cuts give (`buildDropRegions` is tested to agree). */
const hover = (extra: Partial<MergeHover> = {}): MergeHover => {
	const x = extra.x ?? 0;
	const slot = rawSlotAt(x, strip().spans);
	return { x, y: 50, region: `slot:${slot}`, count: 1, pinned: false, ...extra };
};

describe('hoverSlot', () => {
	const model = strip();
	it('uses the slot of the region the plugin found', () => {
		expect(hoverSlot(hover({ region: 'slot:2', x: 9999 }), model)).toBe(2);
		expect(hoverSlot(hover({ region: 'strip' }), model)).toBe(3);
		expect(hoverSlot(hover({ region: 'slot:40' }), model)).toBe(3);
	});
	it('means the end when no region is under the pointer, since a release appends then', () => {
		expect(hoverSlot(hover({ x: 60, region: null }), model)).toBe(3);
		expect(hoverSlot(hover({ x: 60, y: 300, region: null }), model)).toBe(3);
	});
	it('reads a region it does not know as no region', () => {
		expect(hoverSlot(hover({ region: 'other', x: 60 }), model)).toBe(3);
	});
});

describe('landingFor', () => {
	it('puts the line on the edge where the tabs land, from the plain pointer or from the region, alike', () => {
		const model = strip();
		const fromPointer = landingFor(hover({ x: 190 }), model);
		const fromRegion = landingFor(hover({ region: 'slot:2' }), model);
		expect(fromPointer).toEqual(fromRegion);
		expect(fromPointer).toMatchObject({ raw: 2, landed: 2, position: 3, edge: 220 });
	});

	it('keeps the line on the side of the pinned boundary the tabs belong to', () => {
		const model = strip([tab(1, true), tab(2), tab(3)]);
		expect(landingFor(hover({ region: 'slot:0' }), model)).toMatchObject({ landed: 1, edge: 120 });
		expect(landingFor(hover({ region: 'slot:3', pinned: true }), model)).toMatchObject({
			landed: 1,
		});
	});
});

function harness(model: StripMeasure | null = strip()) {
	const store = createMergeLandingStore();
	const said: string[] = [];
	const timers: { run: () => void; ms: number; live: boolean }[] = [];
	let current = model;
	const landing = createMergeLanding({
		store,
		announce: (text) => said.push(text),
		measure: () => current,
		setTimer: (run, ms) => {
			const timer = { run, ms, live: true };
			timers.push(timer);
			return timer;
		},
		clearTimer: (timer) => {
			(timer as { live: boolean }).live = false;
		},
	});
	const fire = () => {
		for (const timer of timers) {
			if (timer.live) {
				timer.live = false;
				timer.run();
			}
		}
	};
	return {
		store,
		said,
		landing,
		timers,
		fire,
		setModel: (next: StripMeasure | null) => (current = next),
	};
}

describe('the landing state', () => {
	it('shows the line in the tablist own coordinates', () => {
		const h = harness();
		h.landing.hover(hover({ x: 190 }));
		expect(h.store.getState().view).toEqual({ left: 200, position: 3, count: 1 });
	});

	it('moves the line when the slot changes and keeps the same state when it does not', () => {
		const h = harness();
		h.landing.hover(hover({ x: 60 }));
		const first = h.store.getState();
		h.landing.hover(hover({ x: 70 }));
		expect(h.store.getState()).toBe(first);
		h.landing.hover(hover({ x: 160 }));
		expect(h.store.getState().view).toMatchObject({ position: 2 });
	});

	it('clears on leave and on clear', () => {
		const h = harness();
		h.landing.hover(hover());
		h.landing.leave();
		expect(h.store.getState().view).toBeNull();
		h.landing.hover(hover());
		h.landing.clear();
		expect(h.store.getState().view).toBeNull();
	});

	it('clears when no hover has refreshed it for the stale time, and a refresh restarts the wait', () => {
		const h = harness();
		h.landing.hover(hover({ x: 60 }));
		expect(h.timers.at(-1)!.ms).toBe(LANDING_STALE_MS);
		h.landing.hover(hover({ x: 160 }));
		// The first timer was cancelled by the refresh; only one is live.
		expect(h.timers.filter((timer) => timer.live)).toHaveLength(1);
		h.fire();
		expect(h.store.getState().view).toBeNull();
		expect(h.timers.filter((timer) => timer.live)).toHaveLength(0);
	});

	it('shows nothing, and says nothing, with no strip to show on', () => {
		const h = harness(null);
		h.landing.hover(hover());
		expect(h.store.getState().view).toBeNull();
		expect(h.said).toEqual([]);
	});

	it('says once per visit where the tab would land, not on every move', () => {
		const h = harness();
		h.landing.hover(hover({ x: 60 }));
		h.landing.hover(hover({ x: 160 }));
		h.landing.hover(hover({ x: 260 }));
		expect(h.said).toEqual(['A tab is being dragged here: release to add it at position 1']);
		// Leaving and coming back is a new visit.
		h.landing.leave();
		h.landing.hover(hover({ x: 260 }));
		expect(h.said).toHaveLength(2);
		expect(h.said[1]).toBe('A tab is being dragged here: release to add it at position 3');
	});

	it('says how many tabs a unit brings', () => {
		const h = harness();
		h.landing.hover(hover({ count: 3, region: 'slot:1' }));
		expect(h.said).toEqual(['3 tabs are being dragged here: release to add them from position 2']);
	});

	it('is back to a fresh visit after the stale clear', () => {
		const h = harness();
		h.landing.hover(hover());
		h.fire();
		h.landing.hover(hover());
		expect(h.said).toHaveLength(2);
	});
});
