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
import { createTearOff, HIT_POLL_MS, WINDOWS_TTL_MS, type TearOff } from './tearOff';

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
async function setup(
	features: ConstructorParameters<typeof FakeTearoffClient>[0] = {},
	options: { hitPollMs?: number } = {},
) {
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
		hitPollMs: options.hitPollMs,
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
	it('shows the card and the pill where the pointer left, on every platform', async () => {
		const h = await setup();
		const source = h.source({ tab: h.ids.a! });
		const pill = h.hook.update(INSIDE, source);
		expect(pill).toMatchObject({ kind: 'window', text: 'Release to open in a new window' });
		expect(h.card.getState().payload).toEqual({ title: 'a' });
		// With no ghost to start (Wayland without a toplevel drag) the card is all there is.
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

	it('stays while the pointer is out of the window and hides when the drag comes back in', async () => {
		const h = await setup();
		const source = h.source({ tab: h.ids.a! });
		h.hook.update(INSIDE, source);
		h.hook.update(OUTSIDE, source);
		expect(h.card.getState().payload).not.toBeNull();
		h.hook.leave();
		expect(h.card.getState().payload).toBeNull();
	});
});

describe('the ghost', () => {
	it('starts once when the pointer leaves the window, and takes the card over', async () => {
		const h = await setup({ ghost: true, cursorFollow: true });
		const source = h.source({ tab: h.ids.a! });
		// The engine calls the hook only once the pointer has left: even at its last position in the window.
		h.hook.update(INSIDE, source);
		expect(h.client.calls).toEqual(['begin']);
		h.hook.update(OUTSIDE, source);
		h.hook.update({ x: 1600, y: 420 }, source);
		await settle();
		expect(h.client.begins).toHaveLength(1);
		expect(h.client.begins[0]).toMatchObject({
			payload: { title: 'a', label: 'Release to open in a new window' },
			grabOffset: { x: 48, y: 20 },
			size: { width: 240, height: 84 },
		});
		// The ghost is the one drawn, not the card.
		expect(h.card.getState().payload).toBeNull();
		expect(h.client.calls.filter((call) => call === 'begin')).toHaveLength(1);
	});

	it('says what a release would do when the cursor is over another window strip', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.hit = { window: 'main-2', region: 'strip', x: 10, y: 10 };
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		expect(h.client.updates.at(-1)).toMatchObject({ label: 'Release to merge into music' });
		const pill = h.hook.update(OUTSIDE, source);
		expect(pill).toMatchObject({
			kind: 'merge',
			text: 'Release to merge into music',
			undrawn: true,
		});
		// The cursor leaves the strip again: the label goes back.
		h.client.hit = null;
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		expect(h.client.updates.at(-1)).toMatchObject({ label: 'Release to open in a new window' });
	});

	it('does not remember a failed window list, and refreshes a good one after a moment', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.hit = { window: 'main-2', region: 'strip', x: 10, y: 10 };
		const list = vi.spyOn(h.api, 'listWindows');
		list.mockRejectedValueOnce(new Error('offline'));
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		expect(h.hook.update(OUTSIDE, source)).toMatchObject({ kind: 'window' });
		// The next poll reads again, and the label appears.
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		expect(h.hook.update(OUTSIDE, source)).toMatchObject({ kind: 'merge' });
		const reads = list.mock.calls.length;
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		expect(list.mock.calls.length).toBe(reads);
		// Past the time to live it asks again, so a window that opened or closed is seen.
		h.tick(WINDOWS_TTL_MS + 1);
		h.hook.update(OUTSIDE, source);
		await settle();
		expect(list.mock.calls.length).toBe(reads + 1);
	});

	it('keeps asking while the ghost follows even though the page gets no pointer events', async () => {
		vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
		try {
			const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
			await dragOut(h, { tab: h.ids.a! });
			h.client.hit = { window: 'main-2', region: 'strip', x: 10, y: 10 };
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
		h.client.hit = { window: 'main-1', region: 'strip', x: 10, y: 10 };
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		h.client.hit = { window: 'main-2', region: 'something-else', x: 10, y: 10 };
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
		h.client.report = reportAt(1500, 400, 1, { window: 'main-2', region: 'strip', x: 10, y: 10 });
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
		h.client.report = reportAt(0, 0, 1, { window: 'main-2', region: 'slot:1', x: 10, y: 10 });
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

	it('commits nothing, and says so, for a region of its own window', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.report = reportAt(0, 0, 1, { window: 'main-1', region: 'strip', x: 10, y: 10 });
		await expect(h.hook.drop(OUTSIDE, source)).resolves.toBe(false);
		expect(h.moveTabs).not.toHaveBeenCalled();
		expect(h.said).toEqual(['Drag cancelled']);
	});

	it('opens a new window when the window it was released over has gone', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		h.client.report = reportAt(0, 0, 1, { window: 'main-9', region: 'strip', x: 10, y: 10 });
		await expect(h.hook.drop(OUTSIDE, source)).resolves.toBe(true);
		expect(h.moveTabs.mock.calls[0]![1]).toMatchObject({ kind: 'newWindow' });
		expect(h.said.at(-1)).toBe('Moved a to a new window');
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
		h.client.hit = { window: 'main-2', region: 'strip', x: 10, y: 10 };
		h.tick();
		h.hook.update(OUTSIDE, source);
		await settle();
		// A motionless pointer makes the plugin report a stale cursor; the label stays.
		h.client.fireStale(true);
		expect(h.hook.update(OUTSIDE, source)).toMatchObject({ kind: 'merge' });
		h.client.report = {
			...reportAt(1500, 400, 1, { window: 'main-2', region: 'strip', x: 10, y: 10 }),
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
		// Inside the window nothing tears off; out of it, a release opens a window.
		move(400, 300);
		expect(h.client.calls).toEqual([]);
		move(-20, 300);
		up(-20, 300);
		await settle();
		await settle();
		expect(h.said.at(-1)).toBe('Moved a to a new window');
		await h.refresh();
		expect(h.store.window('main-1')!.tabs.map((tab) => tab.id)).not.toContain(h.ids.a);
	});
});

describe('telling the other window where the ghost is', () => {
	const over = (region: string, window = 'main-2') => ({ window, region, x: 120, y: 12 });

	async function hover(
		h: Harness,
		source: TabDragSource,
		hit: ReturnType<typeof over> | null,
		ms?: number,
	) {
		h.client.hit = hit;
		h.tick(ms);
		h.hook.update(OUTSIDE, source);
		await settle();
	}

	it('sends the region, the pointer and what travels when the ghost is over another strip', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		await hover(h, source, over('slot:1'));
		expect(h.client.hoverSent).toEqual([
			{
				window: 'main-2',
				hover: { x: 120, y: 12, region: 'slot:1', count: 1, pinned: false },
			},
		]);
	});

	it('says how many tabs a pair or a group brings, and that pinned ones are pinned', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		await h.api.joinPair([h.ids.a!, h.ids.b!], 'sideBySide');
		await h.api.pinTab(h.ids.a!, true);
		await h.refresh();
		const source = await dragOut(h, { tab: h.ids.a! });
		await hover(h, source, over('strip'));
		expect(h.client.hoverSent[0]!.hover).toMatchObject({ count: 2, pinned: true });
	});

	it('sends only when the slot changes, however long the ghost stays and however it moves inside a region', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		await hover(h, source, over('slot:1'));
		await hover(h, source, { ...over('slot:1'), x: 130 });
		await hover(h, source, { ...over('slot:1'), x: 140 });
		expect(h.client.hoverSent).toHaveLength(1);
		await hover(h, source, over('slot:2'));
		expect(h.client.hoverSent.map((sent) => sent.hover.region)).toEqual(['slot:1', 'slot:2']);
		expect(h.client.leavesSent).toEqual([]);
	});

	it('repeats the hover every 250 ms while the ghost stays, so the other window keeps showing it', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		await hover(h, source, over('slot:1'));
		await hover(h, source, over('slot:1'), 100);
		await hover(h, source, over('slot:1'), 100);
		expect(h.client.hoverSent).toHaveLength(1);
		await hover(h, source, over('slot:1'), 100);
		expect(h.client.hoverSent).toHaveLength(2);
		expect(h.client.hoverSent[1]!.hover.region).toBe('slot:1');
	});

	it('sends no faster than every 50 ms, and catches up at the next poll', async () => {
		// A poll interval shorter than the send interval, so the send gate is the one that holds.
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true }, { hitPollMs: 10 });
		const source = await dragOut(h, { tab: h.ids.a! });
		await hover(h, source, over('slot:1'), 11);
		await hover(h, source, over('slot:2'), 20);
		expect(h.client.hoverSent.map((sent) => sent.hover.region)).toEqual(['slot:1']);
		await hover(h, source, over('slot:2'), 35);
		expect(h.client.hoverSent.map((sent) => sent.hover.region)).toEqual(['slot:1', 'slot:2']);
	});

	it('tells the first window it has left when the ghost moves to another, and again when it moves off every strip', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const third = new FakeTabsApi(h.store, 'main-3');
		await third.openTab(fileLocation('/home/test/third'));
		const source = await dragOut(h, { tab: h.ids.a! });
		await hover(h, source, over('slot:1'));
		await hover(h, source, over('strip', 'main-3'));
		expect(h.client.leavesSent).toEqual(['main-2']);
		expect(h.client.hoverSent.map((sent) => sent.window)).toEqual(['main-2', 'main-3']);
		await hover(h, source, null);
		expect(h.client.leavesSent).toEqual(['main-2', 'main-3']);
		// Nothing more to leave.
		await hover(h, source, null);
		expect(h.client.leavesSent).toHaveLength(2);
	});

	it('does not tell its own window, or a region it cannot read', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		await hover(h, source, over('strip', 'main-1'));
		await hover(h, source, over('something-else'));
		expect(h.client.hoverSent).toEqual([]);
	});

	it('tells the window it has left when the drag is cancelled by coming back, and when it times out', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		const stop = h.hook.connect();
		await hover(h, source, over('slot:1'));
		h.hook.leave();
		await settle();
		expect(h.client.leavesSent).toEqual(['main-2']);

		const again = await dragOut(h, { tab: h.ids.b! });
		await hover(h, again, over('slot:1'));
		h.client.fireTimeout();
		await settle();
		expect(h.client.leavesSent).toEqual(['main-2', 'main-2']);
		stop();
	});

	it('tells the window it has left when the drag is released, and merges at the slot that was shown', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		await hover(h, source, over('slot:1'));
		// The pointer moved on a little before the release: the line still showed slot 1.
		h.client.report = reportAt(0, 0, 1, over('slot:2'));
		await expect(h.hook.drop(OUTSIDE, source)).resolves.toBe(true);
		expect(h.client.leavesSent).toEqual(['main-2']);
		expect(h.moveTabs.mock.calls[0]![1]).toEqual({
			kind: 'existingWindow',
			label: 'main-2',
			index: 1,
		});
	});

	it('merges where the cursor is when it is over another window than the one that was shown', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const third = new FakeTabsApi(h.store, 'main-3');
		await third.openTab(fileLocation('/home/test/third'));
		const source = await dragOut(h, { tab: h.ids.a! });
		await hover(h, source, over('slot:1'));
		h.client.report = reportAt(0, 0, 1, over('slot:0', 'main-3'));
		await h.hook.drop(OUTSIDE, source);
		expect(h.moveTabs.mock.calls[0]![1]).toEqual({
			kind: 'existingWindow',
			label: 'main-3',
			index: 0,
		});
	});

	it('shows nothing and sends nothing for a release that is not over a strip', async () => {
		const h = await setup({ ghost: true, cursorFollow: true, hitTest: true });
		const source = await dragOut(h, { tab: h.ids.a! });
		await h.hook.drop(OUTSIDE, source);
		expect(h.client.hoverSent).toEqual([]);
		expect(h.client.leavesSent).toEqual([]);
	});
});
