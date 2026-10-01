// Verifies the new-window phase over a fake plugin and a fake session: the card, the ghost, merges, placement and the fallbacks
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent } from '@testing-library/react';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createDragSession } from '../dnd/dragSession';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { fileLocation } from '../services/fakeVfsClient';
import { FakeTearoffClient, reportAt } from '../services/fakeTearoffClient';
import type { StripMeasure } from './dragLayout';
import { onNotice } from './notices';
import {
	createTabDragHandlers,
	describeTabDrag,
	type TabDragSource,
	type TabDragTarget,
} from './tabDrag';
import { createTearCardStore } from './tearOffCardModel';
import { createTearOff, HIT_POLL_MS, type TearOff } from './tearOff';

const VIEW = { width: 1000, height: 700 };
const STRIP = { left: 0, top: 0, right: 1000, bottom: 30 };

function layout(snapshot: SessionSnapshot): StripMeasure {
	return {
		spans: snapshot.tabs.map((_, at) => ({ left: at * 100, right: at * 100 + 100 })),
		chips: [],
		strip: STRIP,
		tablistLeft: 0,
		area: null,
	};
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** `main-1` holds tabs a, b, c, d; `main-2` holds docs and music. */
async function setup(features: ConstructorParameters<typeof FakeTearoffClient>[0] = {}) {
	const store = new FakeTabsStore({ policy: { closeWindowOnLastTab: true } });
	const api = new FakeTabsApi(store, 'main-1');
	const other = new FakeTabsApi(store, 'main-2');
	const ids: Record<string, TabId> = {};
	for (const name of ['a', 'b', 'c', 'd']) {
		ids[name] = await api.openTab(fileLocation(`/home/test/${name}`));
	}
	await other.openTab(fileLocation('/home/test/docs'));
	await other.openTab(fileLocation('/home/test/music'));
	let snapshot = await api.getSnapshot();
	const refresh = async () => {
		snapshot = await api.getSnapshot();
	};
	const client = new FakeTearoffClient(features);
	const card = createTearCardStore();
	const order: string[] = [];
	const said: string[] = [];
	let cancelled = 0;
	let clock = 1000;
	const moveTabs = vi.spyOn(api, 'moveTabs');
	const hook: TearOff = createTearOff({
		client,
		features: () => client.featureSet,
		api,
		snapshot: () => snapshot,
		flush: async (tab) => {
			order.push(`flush:${tab}`);
		},
		announce: (text) => said.push(text),
		cancelDrag: () => {
			cancelled++;
		},
		card,
		viewport: () => VIEW,
		frameMargin: () => 8,
		now: () => clock,
	});
	moveTabs.mockImplementation(async (what, to) => {
		order.push('move');
		return FakeTabsApi.prototype.moveTabs.call(api, what, to);
	});
	const source = (what: { tab: TabId } | { group: number }): TabDragSource =>
		describeTabDrag(snapshot, layout(snapshot), what)!;
	return {
		store,
		api,
		other,
		ids,
		client,
		card,
		hook,
		order,
		said,
		moveTabs,
		source,
		refresh,
		cancelled: () => cancelled,
		tick: (ms = HIT_POLL_MS + 1) => {
			clock += ms;
		},
	};
}

type Harness = Awaited<ReturnType<typeof setup>>;

const OUTSIDE = { x: 1500, y: 400 };
const INSIDE = { x: 400, y: 300 };

/** Moves out of the window and lets the ghost start. */
async function dragOut(h: Harness, what: { tab: TabId } | { group: number }) {
	const source = h.source(what);
	h.hook.update(OUTSIDE, source);
	await settle();
	return source;
}

let notices: string[];
let stop: () => void;
beforeEach(() => {
	notices = [];
	stop = onNotice((text) => notices.push(text));
});
afterEach(() => stop());

describe('the in-page card', () => {
	it('follows the pointer inside the window with the pill, on every platform', async () => {
		const h = await setup();
		const source = h.source({ tab: h.ids.a! });
		const pill = h.hook.update(INSIDE, source);
		expect(pill).toMatchObject({ kind: 'window', text: 'Release to open in a new window' });
		expect(h.card.getState().payload).toEqual({ title: 'a' });
		// Nothing asks the plugin for a ghost while the pointer is over the page.
		expect(h.client.calls).toEqual([]);
	});

	it('shows a count for a pair and for a group', async () => {
		const h = await setup();
		await h.api.joinPair([h.ids.a!, h.ids.b!], 'sideBySide');
		const group = await h.api.createGroup([h.ids.c!, h.ids.d!], 'Work');
		await h.refresh();
		h.hook.update(INSIDE, h.source({ tab: h.ids.a! }));
		expect(h.card.getState().payload).toEqual({ title: 'a', count: 2 });
		h.hook.update(INSIDE, h.source({ group }));
		expect(h.card.getState().payload).toEqual({ title: 'c', count: 2 });
	});

	it('hides when the pointer leaves the window and when the drag goes back to the strip', async () => {
		const h = await setup();
		const source = h.source({ tab: h.ids.a! });
		h.hook.update(INSIDE, source);
		h.hook.update(OUTSIDE, source);
		expect(h.card.getState().payload).toBeNull();
		h.hook.update(INSIDE, source);
		expect(h.card.getState().payload).not.toBeNull();
		h.hook.leave();
		expect(h.card.getState().payload).toBeNull();
	});
});

describe('the ghost', () => {
	it('starts once when the pointer leaves the window, and takes the card over', async () => {
		const h = await setup({ ghost: true, cursorFollow: true });
		const source = h.source({ tab: h.ids.a! });
		h.hook.update(INSIDE, source);
		expect(h.client.calls).toEqual([]);
		h.hook.update(OUTSIDE, source);
		h.hook.update({ x: 1600, y: 420 }, source);
		await settle();
		expect(h.client.begins).toHaveLength(1);
		expect(h.client.begins[0]).toMatchObject({
			payload: { title: 'a', label: 'Release to open in a new window' },
			grabOffset: { x: 48, y: 20 },
			size: { width: 240, height: 84 },
		});
		// Back over the page, the ghost is still the one drawn.
		h.hook.update(INSIDE, source);
		expect(h.card.getState().payload).toBeNull();
		expect(h.client.calls.filter((call) => call === 'begin')).toHaveLength(1);
	});

	it('says what a release would do when the cursor is over another window strip', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.hit = { window: 'main-2', region: 'strip' };
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		expect(h.client.updates.at(-1)).toMatchObject({ label: 'Release to merge into music' });
		const pill = h.hook.update(OUTSIDE, source);
		expect(pill).toMatchObject({ kind: 'merge', text: 'Release to merge into music' });
		// The cursor leaves the strip again: the label goes back.
		h.client.hit = null;
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		expect(h.client.updates.at(-1)).toMatchObject({ label: 'Release to open in a new window' });
	});

	it('keeps asking while the ghost follows even though the page gets no pointer events', async () => {
		vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
		try {
			const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
			await dragOut(h, { tab: h.ids.a! });
			h.client.hit = { window: 'main-2', region: 'strip' };
			h.tick();
			vi.advanceTimersByTime(HIT_POLL_MS);
			await settle();
			expect(h.client.updates.at(-1)).toMatchObject({ label: 'Release to merge into music' });
			// Putting the ghost away stops the asking.
			h.hook.leave();
			const asked = h.client.calls.filter((call) => call === 'hitTest').length;
			h.tick();
			vi.advanceTimersByTime(HIT_POLL_MS * 5);
			expect(h.client.calls.filter((call) => call === 'hitTest')).toHaveLength(asked);
		} finally {
			vi.useRealTimers();
		}
	});

	it('does not ask where the cursor is more often than the poll interval', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.tick();
		for (let i = 0; i < 5; i++) h.hook.update(OUTSIDE, source);
		await settle();
		expect(h.client.calls.filter((call) => call === 'hitTest')).toHaveLength(1);
	});

	it('does not say merge for a region of its own window or one it cannot read', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.hit = { window: 'main-1', region: 'strip' };
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		h.client.hit = { window: 'main-2', region: 'something-else' };
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		expect(h.hook.update(OUTSIDE, source)).toMatchObject({ kind: 'window' });
	});

	it('puts the ghost away when the pointer comes back to the strip', async () => {
		const h = await setup({ ghost: true, cursorFollow: true });
		await dragOut(h, { tab: h.ids.a! });
		h.hook.leave();
		expect(h.client.calls).toContain('end:cancel');
		expect(h.client.calls).not.toContain('end:drop');
		// A second leave has nothing to put away.
		h.hook.leave();
		expect(h.client.calls.filter((call) => call === 'end:cancel')).toHaveLength(1);
	});

	it('undoes a start that was still in flight when the drag moved on', async () => {
		const h = await setup({ ghost: true, cursorFollow: true });
		let open!: () => void;
		h.client.gate = new Promise<void>((resolve) => (open = resolve));
		h.hook.update(OUTSIDE, h.source({ tab: h.ids.a! }));
		h.hook.leave();
		// `leave` saw no running ghost, but the plugin starts one once the call lands.
		expect(h.client.calls).toEqual(['begin']);
		open();
		await settle();
		expect(h.client.calls).toEqual(['begin', 'end:cancel']);
	});

	it('falls back to the card when the plugin has no ghost to show', async () => {
		const h = await setup({ ghost: true, cursorFollow: true });
		h.client.beginResult = 'noGhost';
		const source = await dragOut(h, { tab: h.ids.a! });
		h.hook.update(INSIDE, source);
		expect(h.card.getState().payload).toEqual({ title: 'a' });
		h.hook.update(OUTSIDE, source);
		expect(h.client.calls.filter((call) => call === 'begin')).toHaveLength(1);
	});
});

describe('a release', () => {
	it('opens a window under the cursor, flushing the hints first', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, windowPosition: true });
		const source = await dragOut(h, { tab: h.ids.b! });
		h.client.report = reportAt(1500, 400, 2);
		await expect(h.hook.drop(OUTSIDE, source)).resolves.toBe(true);
		expect(h.moveTabs).toHaveBeenCalledWith(
			{ kind: 'tabs', value: [h.ids.b] },
			{
				kind: 'newWindow',
				label: null,
				// Cursor (3000, 800) less (48 + 8, 20 + 8) logical at 2x; the window keeps this one's size.
				geometry: { x: 2888, y: 744, width: 2000, height: 1400, maximised: false },
			},
		);
		expect(h.order).toEqual([`flush:${h.ids.b}`, 'move']);
		expect(h.client.calls).toContain('end:drop');
		expect(h.said).toEqual(['Moved b to a new window']);
		await h.refresh();
		expect(h.store.window('main-1')!.tabs.map((tab) => tab.id)).not.toContain(h.ids.b);
		expect(h.card.getState().payload).toBeNull();
	});

	it('opens a window with no geometry where the cursor is stale or missing', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, windowPosition: true });
		let source = await dragOut(h, { tab: h.ids.a! });
		h.client.report = { ...reportAt(1500, 400, 2), cursorStale: true };
		await h.hook.drop(OUTSIDE, source);
		expect(h.moveTabs.mock.calls[0]![1]).toEqual({
			kind: 'newWindow',
			label: null,
			geometry: null,
		});

		await h.refresh();
		source = await dragOut(h, { tab: h.ids.c! });
		h.client.report = { cursor: null, scaleFactor: 2, cursorStale: false, hit: null };
		await h.hook.drop(OUTSIDE, source);
		expect(h.moveTabs.mock.calls[1]![1]).toEqual({
			kind: 'newWindow',
			label: null,
			geometry: null,
		});
	});

	it('does not place a window where the system cannot', async () => {
		const h = await setup({ ghost: true, cursorFollow: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.report = reportAt(1500, 400, 1);
		await h.hook.drop(OUTSIDE, source);
		expect(h.moveTabs.mock.calls[0]![1]).toMatchObject({ kind: 'newWindow', geometry: null });
	});

	it('merges at the end of the strip it was released over', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true, windowPosition: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.report = reportAt(1500, 400, 1, { window: 'main-2', region: 'strip' });
		await expect(h.hook.drop(OUTSIDE, source)).resolves.toBe(true);
		expect(h.moveTabs).toHaveBeenCalledWith(
			{ kind: 'tabs', value: [h.ids.a] },
			{ kind: 'existingWindow', label: 'main-2', index: 2 },
		);
		expect(h.order).toEqual([`flush:${h.ids.a}`, 'move']);
		expect(h.said).toEqual(['Merged a into music']);
		await h.refresh();
		expect(h.store.window('main-2')!.tabs.map((tab) => tab.location.display ?? '')).toHaveLength(3);
	});

	it('merges next to the tab whose half it was released over', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.report = reportAt(0, 0, 1, { window: 'main-2', region: 'slot:1' });
		await h.hook.drop(OUTSIDE, source);
		expect(h.moveTabs.mock.calls[0]![1]).toEqual({
			kind: 'existingWindow',
			label: 'main-2',
			index: 1,
		});
		await h.refresh();
		const tabs = h.store.window('main-2')!.tabs;
		expect(tabs[1]!.id).toBe(h.ids.a);
	});

	it('commits nothing for a region of its own window or of a window that has gone', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		let source = await dragOut(h, { tab: h.ids.a! });
		h.client.report = reportAt(0, 0, 1, { window: 'main-1', region: 'strip' });
		await expect(h.hook.drop(OUTSIDE, source)).resolves.toBe(false);
		source = await dragOut(h, { tab: h.ids.a! });
		h.client.report = reportAt(0, 0, 1, { window: 'main-9', region: 'strip' });
		await expect(h.hook.drop(OUTSIDE, source)).resolves.toBe(false);
		expect(h.moveTabs).not.toHaveBeenCalled();
	});

	it('moves a group or a pair as one, and a single half of a pair alone', async () => {
		const h = await setup();
		const pair = await h.api.joinPair([h.ids.a!, h.ids.b!], 'sideBySide');
		const group = await h.api.createGroup([h.ids.c!, h.ids.d!], 'Work');
		await h.refresh();
		await h.hook.drop(OUTSIDE, h.source({ tab: h.ids.a! }));
		expect(h.moveTabs.mock.calls[0]![0]).toEqual({ kind: 'pair', value: pair });
		expect(h.said.at(-1)).toMatch(/^Moved .+ to a new window$/);

		await h.refresh();
		await h.hook.drop(OUTSIDE, h.source({ group }));
		expect(h.moveTabs.mock.calls[1]![0]).toEqual({ kind: 'group', value: group });
		expect(h.said.at(-1)).toBe('Moved Work to a new window');
	});

	it('moves one pane of a pair when the source is that tab alone', async () => {
		const h = await setup();
		await h.api.joinPair([h.ids.a!, h.ids.b!], 'sideBySide');
		await h.refresh();
		const whole = h.source({ tab: h.ids.a! });
		await h.hook.drop(OUTSIDE, { ...whole, unit: [h.ids.b!], lead: h.ids.b! });
		expect(h.moveTabs.mock.calls[0]![0]).toEqual({ kind: 'tabs', value: [h.ids.b] });
		expect(h.order).toEqual([`flush:${h.ids.b}`, 'move']);
	});

	it('degrades on Wayland: no ghost, no cursor, a window the compositor places', async () => {
		const h = await setup();
		const source = h.source({ tab: h.ids.a! });
		h.hook.update(OUTSIDE, source);
		h.hook.update(INSIDE, source);
		await expect(h.hook.drop(INSIDE, source)).resolves.toBe(true);
		expect(h.client.calls).toEqual([]);
		expect(h.moveTabs).toHaveBeenCalledWith(
			{ kind: 'tabs', value: [h.ids.a] },
			{ kind: 'newWindow', label: null, geometry: null },
		);
		expect(h.said).toEqual(['Moved a to a new window']);
	});

	it('says so, and moves nothing, when the session refuses the move', async () => {
		const h = await setup();
		h.moveTabs.mockRejectedValueOnce({ kind: 'tooManyWindows', limit: 12 });
		await expect(h.hook.drop(OUTSIDE, h.source({ tab: h.ids.a! }))).resolves.toBe(false);
		expect(notices).toEqual(['Waypoint cannot open more than 12 windows. Close one first.']);
		expect(h.said).toEqual([]);
	});

	it('asks the plugin where a release landed even if the drag never left the window', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, windowPosition: true });
		const source = h.source({ tab: h.ids.a! });
		h.hook.update(INSIDE, source);
		h.client.report = reportAt(400, 300, 1);
		await h.hook.drop(INSIDE, source);
		expect(h.client.calls).toEqual(['end:drop']);
		expect(h.moveTabs.mock.calls[0]![1]).toMatchObject({
			kind: 'newWindow',
			geometry: { x: 400 - 56, y: 300 - 28 },
		});
	});
});

describe('the plugin events', () => {
	it('ends a drag that ran too long, cancels it and says so', async () => {
		const h = await setup({ ghost: true, cursorFollow: true });
		const stopListening = h.hook.connect();
		await dragOut(h, { tab: h.ids.a! });
		h.client.fireTimeout();
		expect(h.cancelled()).toBe(1);
		expect(h.said).toEqual(['Drag ended because it ran too long']);
		expect(h.card.getState().payload).toBeNull();
		// The plugin has already put the ghost away; the engine's cancel does not do it twice.
		h.hook.leave();
		expect(h.client.calls).not.toContain('end:cancel');
		stopListening();
	});

	it('stops listening, and puts a running ghost away, when the window goes', async () => {
		const h = await setup({ ghost: true, cursorFollow: true });
		const stopListening = h.hook.connect();
		await dragOut(h, { tab: h.ids.a! });
		stopListening();
		expect(h.client.calls).toContain('end:cancel');
		h.client.fireTimeout();
		expect(h.cancelled()).toBe(0);
	});

	it('keeps the merge label while the pointer is held still, and does not place a window by a stale cursor', async () => {
		const h = await setup({
			ghost: true,
			cursorFollow: true,
			hitTest: true,
			windowPosition: true,
		});
		const stopListening = h.hook.connect();
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.hit = { window: 'main-2', region: 'strip' };
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		// A motionless pointer makes the plugin report a stale cursor; the label stays.
		h.client.fireStale(true);
		expect(h.hook.update(OUTSIDE, source)).toMatchObject({ kind: 'merge' });
		h.client.report = {
			...reportAt(1500, 400, 1, { window: 'main-2', region: 'strip' }),
			cursorStale: true,
		};
		await h.hook.drop(OUTSIDE, source);
		expect(h.moveTabs.mock.calls[0]![1]).toMatchObject({ kind: 'existingWindow' });
		h.client.report = { ...reportAt(1500, 400, 1), cursorStale: true };

		await h.refresh();
		const again = await dragOut(h, { tab: h.ids.c! });
		await h.hook.drop(OUTSIDE, again);
		expect(h.moveTabs.mock.calls[1]![1]).toEqual({
			kind: 'newWindow',
			label: null,
			geometry: null,
		});
		stopListening();
	});
});

describe('over the drag engine', () => {
	/** The engine and the hook as the strip has them, pressing tab `a` at the strip. */
	async function engine(features: ConstructorParameters<typeof FakeTearoffClient>[0]) {
		const h = await setup(features);
		const session = createDragSession<TabDragSource, TabDragTarget>({
			startThresholdPx: 4,
			reducedMotion: () => true,
			announce: (text) => h.said.push(text),
			sameTarget: (a, b) => JSON.stringify(a) === JSON.stringify(b),
		});
		let snapshot = await h.api.getSnapshot();
		const press = () => {
			snapshot = snapshotOf(h);
			session.begin(
				{ pointerId: 1, clientX: 50, clientY: 15, source: h.source({ tab: h.ids.a! }) },
				createTabDragHandlers({
					api: h.api,
					snapshot: () => snapshot,
					announce: (text) => h.said.push(text),
					requestRename: () => {},
					tearOff: h.hook,
				}),
			);
		};
		const move = (x: number, y: number) =>
			fireEvent.pointerMove(window, { clientX: x, clientY: y, pointerId: 1 });
		const up = (x: number, y: number) =>
			fireEvent.pointerUp(window, { clientX: x, clientY: y, pointerId: 1 });
		return { h, session, press, move, up };
	}
	const snapshotOf = (h: Harness) => h.store.snapshot('main-1');

	it('cancels by returning to the strip, and by Escape', async () => {
		const { h, session, press, move } = await engine({ ghost: true, cursorFollow: true });
		press();
		move(60, 15);
		move(OUTSIDE.x, OUTSIDE.y);
		await settle();
		expect(session.store.getState().pill?.text).toBe('Release to open in a new window');
		expect(h.client.calls).toEqual(['begin']);
		move(70, 20);
		expect(h.client.calls).toEqual(['begin', 'end:cancel']);
		expect(session.store.getState().pill?.kind).not.toBe('window');

		move(OUTSIDE.x, OUTSIDE.y);
		await settle();
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(h.client.calls.at(-1)).toBe('end:cancel');
		expect(h.client.calls.filter((call) => call === 'begin')).toHaveLength(2);
		expect(h.said).toContain('Drag cancelled');
		await h.refresh();
		expect(h.store.window('main-1')!.tabs).toHaveLength(4);
	});

	it('opens a window on release outside the strip', async () => {
		const { h, press, move, up } = await engine({});
		press();
		move(60, 15);
		move(400, 300);
		up(400, 300);
		await settle();
		await settle();
		expect(h.said.at(-1)).toBe('Moved a to a new window');
		await h.refresh();
		expect(h.store.window('main-1')!.tabs.map((tab) => tab.id)).not.toContain(h.ids.a);
	});
});
