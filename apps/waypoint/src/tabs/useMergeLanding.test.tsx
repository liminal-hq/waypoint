// Verifies the hooks feeding the landing line: both event sources, the clears, and the measuring of the strip
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, renderHook } from '@testing-library/react';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTearoffClient } from '../services/fakeTearoffClient';
import { createMergeLandingStore } from './mergeLanding';
import { hoverFromPlugin, measureStripLanding, useMergeLanding } from './useMergeLanding';

function tab(id: number, group: number | null = null): TabSnapshot {
	return { id, pinned: false, group, hints: {} } as unknown as TabSnapshot;
}

function snapshotOf(tabs: TabSnapshot[]): SessionSnapshot {
	return { tabs, groups: [], pairs: [], active: tabs[0]?.id ?? null } as unknown as SessionSnapshot;
}

/** A strip at x = 0..800, y = 0..30, tabs 100 wide, plus an optional collapsed chip. */
function mountStrip(count: number) {
	const strip = document.createElement('div');
	strip.setAttribute('data-strip', '');
	const list = document.createElement('div');
	list.setAttribute('role', 'tablist');
	for (let i = 0; i < count; i++) {
		const slot = document.createElement('div');
		slot.setAttribute('data-slot', '');
		slot.setAttribute('data-index', String(i));
		list.append(slot);
	}
	strip.append(list);
	document.body.append(strip);
}

beforeEach(() => {
	vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
		this: HTMLElement,
	) {
		if (this.hasAttribute('data-strip') || this.getAttribute('role') === 'tablist') {
			return { left: 0, right: 800, top: 0, bottom: 30, width: 800, height: 30 } as DOMRect;
		}
		const index = Number(this.getAttribute('data-index') ?? 0);
		const left = index * 100;
		return { left, right: left + 100, top: 0, bottom: 30, width: 100, height: 30 } as DOMRect;
	});
});

afterEach(() => {
	document.body.replaceChildren();
	vi.restoreAllMocks();
});

const tabs = [tab(1), tab(2), tab(3)];
const payload = (count = 1) => ({
	v: 1,
	mode: 'tabs',
	what: { kind: 'tabs', value: [1] },
	tabs: Array.from({ length: count }, (_, i) => 100 + i),
	name: 'x',
	pinned: false,
	source: { window: 'main-1', index: 0 },
});

function mount(client: FakeTearoffClient) {
	mountStrip(3);
	const store = createMergeLandingStore();
	const said: string[] = [];
	const announce = (text: string) => said.push(text);
	const view = renderHook(() => useMergeLanding(client, store, announce, snapshotOf(tabs)));
	return { store, said, ...view };
}

describe('measureStripLanding', () => {
	it('reads the strip, the tabs and the tablist', () => {
		mountStrip(3);
		const measured = measureStripLanding(snapshotOf(tabs))!;
		expect(measured.spans).toEqual([
			{ left: 0, right: 100 },
			{ left: 100, right: 200 },
			{ left: 200, right: 300 },
		]);
		expect(measured.strip).toEqual({ left: 0, right: 800, top: 0, bottom: 30 });
		expect(measured.tablistLeft).toBe(0);
	});

	it('is null where there is no strip', () => {
		expect(measureStripLanding(snapshotOf(tabs))).toBeNull();
	});
});

describe('hoverFromPlugin', () => {
	it('turns the plugin hover into a merge hover with the tab count and pinned flag', () => {
		expect(
			hoverFromPlugin({ window: 'main-2', x: 5, y: 6, region: 'slot:1', payload: payload(2) }),
		).toEqual({ x: 5, y: 6, region: 'slot:1', count: 2, pinned: false });
	});
	it('ignores a payload that is not a tab drag', () => {
		expect(
			hoverFromPlugin({ window: 'main-2', x: 5, y: 6, region: null, payload: { hello: 1 } }),
		).toBeNull();
	});
});

describe('useMergeLanding', () => {
	it('shows the line for a ghost hover and clears it on leave', () => {
		const client = new FakeTearoffClient();
		const { store, said } = mount(client);
		act(() => client.fireMergeHover({ x: 0, y: 0, region: 'slot:1', count: 1, pinned: false }));
		expect(store.getState().view).toEqual({ left: 100, position: 2, count: 1 });
		expect(said).toEqual(['A tab is being dragged here: release to add it at position 2']);
		act(() => client.fireMergeLeave());
		expect(store.getState().view).toBeNull();
	});

	it('shows the line for a window drag hover from the plugin, over the strip and over the rest of the window', () => {
		const client = new FakeTearoffClient();
		const { store } = mount(client);
		act(() =>
			client.fireDragHover({
				window: 'main-2',
				x: 160,
				y: 10,
				region: 'slot:2',
				payload: payload(),
			}),
		);
		expect(store.getState().view).toMatchObject({ left: 200, position: 3 });
		// Over the file area: no region, so the end.
		act(() =>
			client.fireDragHover({ window: 'main-2', x: 160, y: 300, region: null, payload: payload() }),
		);
		expect(store.getState().view).toMatchObject({ left: 300, position: 4 });
		act(() => client.fireDragLeave({ window: 'main-2' }));
		expect(store.getState().view).toBeNull();
	});

	it('ignores a hover of a payload that is not a tab drag', () => {
		const client = new FakeTearoffClient();
		const { store } = mount(client);
		act(() =>
			client.fireDragHover({ window: 'main-2', x: 1, y: 1, region: null, payload: { other: 1 } }),
		);
		expect(store.getState().view).toBeNull();
	});

	it('clears when the payload is dropped here, and when the window loses focus', () => {
		const client = new FakeTearoffClient();
		const { store } = mount(client);
		const hover = { x: 0, y: 0, region: 'slot:1', count: 1, pinned: false };
		act(() => client.fireMergeHover(hover));
		act(() => client.fireDropped({ window: 'main-2', payload: {}, x: 0, y: 0, region: null }));
		expect(store.getState().view).toBeNull();
		act(() => client.fireMergeHover(hover));
		act(() => {
			window.dispatchEvent(new Event('blur'));
		});
		expect(store.getState().view).toBeNull();
	});

	it('clears a hover that is not refreshed', () => {
		vi.useFakeTimers();
		try {
			const client = new FakeTearoffClient();
			const { store } = mount(client);
			act(() => client.fireMergeHover({ x: 0, y: 0, region: 'slot:1', count: 1, pinned: false }));
			expect(store.getState().view).not.toBeNull();
			act(() => {
				vi.advanceTimersByTime(399);
			});
			expect(store.getState().view).not.toBeNull();
			act(() => {
				vi.advanceTimersByTime(2);
			});
			expect(store.getState().view).toBeNull();
		} finally {
			vi.useRealTimers();
		}
	});

	it('clears and stops listening when the window goes', () => {
		const client = new FakeTearoffClient();
		const { store, unmount } = mount(client);
		act(() => client.fireMergeHover({ x: 0, y: 0, region: 'slot:1', count: 1, pinned: false }));
		unmount();
		expect(store.getState().view).toBeNull();
		act(() => client.fireMergeHover({ x: 0, y: 0, region: 'slot:1', count: 1, pinned: false }));
		expect(store.getState().view).toBeNull();
	});
});
