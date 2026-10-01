// Verifies the compositor-moved tear-off (Wayland, `toplevel_drag`) over a fake plugin and a fake session: starting, every way it ends, and the hand-off
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { fileLocation } from '../services/fakeVfsClient';
import { FakeTearoffClient } from '../services/fakeTearoffClient';
import type { ToplevelDragEnded } from '../services/tearoffClient';
import type { StripMeasure } from './dragLayout';
import { onNotice } from './notices';
import { describeTabDrag, type TabDragSource } from './tabDrag';
import { createTearOff, type TearOff } from './tearOff';
import { createTearHandoff } from './tearOffHandoff';
import { createTearCardStore } from './tearOffCard';
import type { TearPayload } from './tearOffPayload';

const VIEW = { width: 1000, height: 700 };
const STRIP = { left: 0, top: 0, right: 1000, bottom: 30 };
const OUT = { x: 400, y: 200 };

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
async function setup() {
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
	const client = new FakeTearoffClient({ toplevelDrag: true });
	const order: string[] = [];
	const said: string[] = [];
	let cancelled = 0;
	const moveTabs = vi.spyOn(api, 'moveTabs');
	moveTabs.mockImplementation(async (what, to) => {
		order.push('move');
		return FakeTabsApi.prototype.moveTabs.call(api, what, to);
	});
	client.holdNextWindow = async (on) => {
		order.push(`hold:${on}`);
		client.holds.push(on);
	};
	const card = createTearCardStore();
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
			order.push('cancelDrag');
		},
		card,
		viewport: () => VIEW,
		frameMargin: () => 8,
	});
	const source = (what: { tab: TabId } | { group: number }): TabDragSource =>
		describeTabDrag(snapshot, layout(snapshot), what)!;
	hook.connect();
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
	};
}

type Harness = Awaited<ReturnType<typeof setup>>;

/** Pulls the unit out of the strip and lets the move and the start run. */
async function tearOut(h: Harness, what: { tab: TabId } | { group: number }) {
	const source = h.source(what);
	h.hook.update(OUT, source);
	await settle();
	return source;
}

function ended(h: Harness, over: Partial<ToplevelDragEnded>): ToplevelDragEnded {
	const call = h.client.toplevelBegins.at(-1)!;
	return {
		seq: 1,
		window: call.windowLabel,
		source: 'main-1',
		outcome: 'cancelled',
		target: null,
		payload: call.payload,
		reason: null,
		...over,
	};
}

let notices: string[];
let stop: () => void;
beforeEach(() => {
	notices = [];
	stop = onNotice((text) => notices.push(text));
});
afterEach(() => stop());

describe('starting', () => {
	it('moves the tab to a window made hidden, then drags that window', async () => {
		const h = await setup();
		const source = h.source({ tab: h.ids.b! });
		const pill = h.hook.update(OUT, source);
		expect(pill).toMatchObject({ kind: 'window', text: 'Release to open in a new window' });
		await settle();
		// Hints first, the window held hidden only around the move, the drag after it.
		expect(h.order).toEqual([`flush:${h.ids.b}`, 'hold:true', 'move', 'hold:false', 'cancelDrag']);
		expect(h.client.toplevelBegins).toHaveLength(1);
		const call = h.client.toplevelBegins[0]!;
		expect(call.windowLabel).toBe('main-3');
		expect(call.grabOffset).toEqual({ x: 400, y: 200 });
		const payload = call.payload as TearPayload;
		expect(payload).toMatchObject({
			v: 1,
			mode: 'tabs',
			what: { kind: 'tabs', value: [h.ids.b] },
			tabs: [h.ids.b],
			name: 'b',
			source: { window: 'main-1', index: 1 },
		});
		await h.refresh();
		expect(h.store.window('main-1')!.tabs.map((tab) => tab.id)).not.toContain(h.ids.b);
		expect(h.store.window('main-3')!.tabs.map((tab) => tab.id)).toEqual([h.ids.b]);
	});

	it('shows neither the card nor the ghost: the real window is the preview', async () => {
		const h = await setup();
		await tearOut(h, { tab: h.ids.a! });
		expect(h.card.getState().payload).toBeNull();
		expect(h.client.calls).not.toContain('begin');
		expect(h.client.calls.filter((call) => call === 'hitTest')).toHaveLength(0);
	});

	it('starts once however many pointer moves follow', async () => {
		const h = await setup();
		const source = h.source({ tab: h.ids.a! });
		for (let i = 0; i < 6; i++) h.hook.update({ x: 400 + i, y: 200 }, source);
		await settle();
		expect(h.client.toplevelBegins).toHaveLength(1);
		expect(h.moveTabs).toHaveBeenCalledTimes(1);
	});

	it('stops the drag engine when the compositor takes the drag', async () => {
		const h = await setup();
		await tearOut(h, { tab: h.ids.a! });
		expect(h.cancelled()).toBe(1);
		expect(h.hook.handsOff()).toBe(true);
	});

	it('keeps the grab inside the window', async () => {
		const h = await setup();
		h.hook.update({ x: 4000, y: -50 }, h.source({ tab: h.ids.a! }));
		await settle();
		expect(h.client.toplevelBegins[0]!.grabOffset).toEqual({ x: 1000, y: 0 });
	});

	it('drags a pair as one unit', async () => {
		const h = await setup();
		const pair = await h.api.joinPair([h.ids.a!, h.ids.b!], 'sideBySide');
		await h.refresh();
		await tearOut(h, { tab: h.ids.a! });
		const payload = h.client.toplevelBegins[0]!.payload as TearPayload;
		expect(payload.what).toEqual({ kind: 'pair', value: pair });
		expect(payload.tabs).toEqual([h.ids.a, h.ids.b]);
		expect(h.order.filter((entry) => entry.startsWith('flush:'))).toHaveLength(2);
		await h.refresh();
		expect(h.store.window('main-3')!.tabs).toHaveLength(2);
	});

	it('drags a group by its chip as one unit', async () => {
		const h = await setup();
		const group = await h.api.createGroup([h.ids.c!, h.ids.d!], 'Work');
		await h.refresh();
		await tearOut(h, { group });
		const payload = h.client.toplevelBegins[0]!.payload as TearPayload;
		expect(payload.what).toEqual({ kind: 'group', value: group });
		expect(payload.tabs).toEqual([h.ids.c, h.ids.d]);
		expect(payload.name).toBe('Work');
		expect(payload.source.index).toBe(2);
	});

	it('takes the window itself when the tabs are all it has', async () => {
		const h = await setup();
		const lone = new FakeTabsApi(h.store, 'main-1');
		for (const name of ['b', 'c', 'd']) await lone.closeTab(h.ids[name]!);
		await h.refresh();
		await tearOut(h, { tab: h.ids.a! });
		expect(h.moveTabs).not.toHaveBeenCalled();
		expect(h.client.holds).toEqual([]);
		const call = h.client.toplevelBegins[0]!;
		expect(call.windowLabel).toBe('main-1');
		expect((call.payload as TearPayload).mode).toBe('window');
	});
});

describe('when it cannot start', () => {
	it('says so and tries nothing more until the pointer is back in the strip', async () => {
		const h = await setup();
		h.client.toplevelBegin = { state: 'failed', reason: 'no button is held' };
		const source = await tearOut(h, { tab: h.ids.a! });
		// The plugin reports the failure to the window it was told to drag.
		h.hook.update(OUT, source);
		h.hook.update(OUT, source);
		await settle();
		expect(h.client.toplevelBegins).toHaveLength(1);
		expect(h.moveTabs).toHaveBeenCalledTimes(1);
		expect(h.hook.handsOff()).toBe(false);
		h.client.fireEnded(ended(h, { outcome: 'failed', reason: 'no button is held' }));
		expect(notices).toEqual(['Could not move the tab.']);
		expect(h.said).toContain('Could not move the tab.');
		expect(h.cancelled()).toBe(1);
		// A fresh drag works again.
		h.hook.leave();
		h.client.toplevelBegin = { state: 'started', reason: null };
		h.hook.update(OUT, h.source({ tab: h.ids.c! }));
		await settle();
		expect(h.client.toplevelBegins).toHaveLength(2);
	});

	it('does not drag, and says why, when the session refuses the new window', async () => {
		const h = await setup();
		h.moveTabs.mockImplementation(async () => {
			h.order.push('move');
			throw new Error('too many windows');
		});
		await tearOut(h, { tab: h.ids.a! });
		expect(h.client.toplevelBegins).toEqual([]);
		// The window is not held hidden for the next one.
		expect(h.order).toEqual([`flush:${h.ids.a}`, 'hold:true', 'move', 'hold:false']);
		expect(notices).toHaveLength(1);
		expect(h.hook.handsOff()).toBe(false);
	});
});

describe('while the compositor has the drag', () => {
	it('ignores the pointer coming back to the strip and a release', async () => {
		const h = await setup();
		const source = await tearOut(h, { tab: h.ids.a! });
		h.hook.leave();
		expect(h.hook.handsOff()).toBe(true);
		await expect(h.hook.drop(OUT, source)).resolves.toBe(true);
		expect(h.moveTabs).toHaveBeenCalledTimes(1);
		expect(h.client.calls).not.toContain('end:drop');
		expect(h.client.calls).not.toContain('end:cancel');
	});

	it('says nothing when the engine is stopped, because the drag is not cancelled', async () => {
		const h = await setup();
		await tearOut(h, { tab: h.ids.a! });
		expect(h.said).toEqual([]);
	});
});

describe('how it ends, for the window that began it', () => {
	const endings: [ToplevelDragEnded['outcome'], string[]][] = [
		['dropped-on-window', []],
		['dropped-elsewhere', ['Moved a to a new window']],
		['cancelled', ['Drag cancelled']],
	];
	for (const [outcome, said] of endings) {
		it(`resets the drag and announces ${outcome}`, async () => {
			const h = await setup();
			await tearOut(h, { tab: h.ids.a! });
			h.client.fireEnded(
				ended(h, { outcome, target: outcome === 'dropped-on-window' ? 'main-2' : null }),
			);
			expect(h.said).toEqual(said);
			expect(h.hook.handsOff()).toBe(false);
			// The engine is stopped again, in case the page missed the start.
			expect(h.cancelled()).toBe(2);
		});
	}

	it('is ready for the next drag after one ends', async () => {
		const h = await setup();
		await tearOut(h, { tab: h.ids.a! });
		h.client.fireEnded(ended(h, { outcome: 'dropped-elsewhere' }));
		await h.refresh();
		await tearOut(h, { tab: h.ids.b! });
		expect(h.client.toplevelBegins).toHaveLength(2);
	});

	it('ignores an end it did not begin', async () => {
		const h = await setup();
		h.client.fireEnded({
			seq: 9,
			window: 'main-2',
			source: 'main-3',
			outcome: 'cancelled',
			target: null,
			payload: null,
			reason: null,
		});
		expect(h.said).toEqual([]);
		expect(h.cancelled()).toBe(0);
	});
});

describe('the hand-off in the window that holds the tabs', () => {
	/** The new window's page: the tabs a and b are in `main-3`, torn from `main-1`. */
	async function torn(units: 'one' | 'whole' = 'one') {
		const h = await setup();
		if (units === 'whole') {
			const lone = new FakeTabsApi(h.store, 'main-1');
			for (const name of ['b', 'c', 'd']) await lone.closeTab(h.ids[name]!);
			await h.refresh();
		}
		await tearOut(h, { tab: h.ids.a! });
		const label = h.client.toplevelBegins[0]!.windowLabel;
		const page = new FakeTabsApi(h.store, label);
		const said: string[] = [];
		const flushed: TabId[] = [];
		const handoff = createTearHandoff({
			client: h.client,
			api: page,
			flush: async (tab) => {
				flushed.push(tab);
			},
			announce: (text) => said.push(text),
		});
		return { h, label, page, said, flushed, handoff };
	}

	it('merges into the window it was dropped on and closes itself', async () => {
		const { h, label, handoff, flushed } = await torn();
		await handoff.resolve(ended(h, { outcome: 'dropped-on-window', target: 'main-2' }));
		expect(h.store.window('main-2')!.tabs.map((tab) => tab.id)).toEqual([
			expect.any(Number),
			expect.any(Number),
			h.ids.a,
		]);
		expect(h.store.window(label)).toBeUndefined();
		expect(flushed).toEqual([h.ids.a]);
	});

	it('merges every tab of a pair, in order', async () => {
		const h = await setup();
		await h.api.joinPair([h.ids.a!, h.ids.b!], 'sideBySide');
		await h.refresh();
		await tearOut(h, { tab: h.ids.a! });
		const label = h.client.toplevelBegins[0]!.windowLabel;
		const handoff = createTearHandoff({
			client: h.client,
			api: new FakeTabsApi(h.store, label),
			flush: async () => {},
			announce: () => {},
		});
		await handoff.resolve(ended(h, { outcome: 'dropped-on-window', target: 'main-2' }));
		const merged = h.store.window('main-2')!;
		expect(merged.tabs.slice(-2).map((tab) => tab.id)).toEqual([h.ids.a, h.ids.b]);
		expect(merged.pairs).toHaveLength(1);
	});

	it('leaves the window where the compositor dropped it', async () => {
		const { h, label, handoff } = await torn();
		await handoff.resolve(ended(h, { outcome: 'dropped-elsewhere' }));
		expect(h.store.window(label)!.tabs.map((tab) => tab.id)).toEqual([h.ids.a]);
	});

	it('returns the tab to where it was when the drag is cancelled', async () => {
		const { h, label, handoff } = await torn();
		await handoff.resolve(ended(h, { outcome: 'cancelled' }));
		await h.refresh();
		expect(h.store.window('main-1')!.tabs.map((tab) => tab.id)).toEqual([
			h.ids.a,
			h.ids.b,
			h.ids.c,
			h.ids.d,
		]);
		expect(h.store.window(label)).toBeUndefined();
	});

	it('returns the tab when the drag fails', async () => {
		const { h, handoff } = await torn();
		await handoff.resolve(ended(h, { outcome: 'failed', reason: 'no button' }));
		expect(h.store.window('main-1')!.tabs.map((tab) => tab.id)).toContain(h.ids.a);
	});

	it('returns a tab from the middle to its place', async () => {
		const h = await setup();
		await tearOut(h, { tab: h.ids.c! });
		const label = h.client.toplevelBegins[0]!.windowLabel;
		const handoff = createTearHandoff({
			client: h.client,
			api: new FakeTabsApi(h.store, label),
			flush: async () => {},
			announce: () => {},
		});
		await handoff.resolve(ended(h, { outcome: 'cancelled' }));
		expect(h.store.window('main-1')!.tabs.map((tab) => tab.id)).toEqual([
			h.ids.a,
			h.ids.b,
			h.ids.c,
			h.ids.d,
		]);
	});

	it('keeps the tabs where they are when the window they came from has gone', async () => {
		const { h, label, handoff } = await torn();
		const source = new FakeTabsApi(h.store, 'main-1');
		for (const name of ['b', 'c', 'd']) await source.closeTab(h.ids[name]!);
		// Closing its last tab closed it.
		expect(h.store.window('main-1')).toBeUndefined();
		await handoff.resolve(ended(h, { outcome: 'cancelled' }));
		expect(h.store.window(label)!.tabs.map((tab) => tab.id)).toEqual([h.ids.a]);
	});

	it('does nothing in the window that began the drag and does not hold the tabs', async () => {
		const { h, handoff } = await torn();
		const source = createTearHandoff({
			client: h.client,
			api: h.api,
			flush: async () => {},
			announce: () => {},
		});
		await source.resolve(ended(h, { outcome: 'dropped-on-window', target: 'main-2' }));
		expect(h.store.window('main-2')!.tabs).toHaveLength(2);
		void handoff;
	});

	it('acts once when the end arrives twice', async () => {
		const { h, handoff } = await torn();
		const result = ended(h, { outcome: 'dropped-on-window', target: 'main-2' });
		await handoff.resolve(result);
		await handoff.resolve(result);
		expect(h.store.window('main-2')!.tabs.filter((tab) => tab.id === h.ids.a)).toHaveLength(1);
	});

	it('merges a whole window into the one it was dropped on, and closes', async () => {
		const { h, handoff } = await torn('whole');
		await handoff.resolve(
			ended(h, { outcome: 'dropped-on-window', target: 'main-2', window: 'main-1' }),
		);
		expect(h.store.window('main-2')!.tabs.map((tab) => tab.id)).toContain(h.ids.a);
	});

	it('leaves a whole window alone when its drag is cancelled', async () => {
		const { h, handoff } = await torn('whole');
		await handoff.resolve(ended(h, { outcome: 'cancelled', window: 'main-1' }));
		expect(h.store.window('main-1')!.tabs.map((tab) => tab.id)).toEqual([h.ids.a]);
	});

	it('reads an end that came before the page was listening', async () => {
		const { h, handoff } = await torn();
		h.client.pendingResult = ended(h, { outcome: 'cancelled' });
		const stop = handoff.connect();
		await settle();
		await settle();
		stop();
		expect(h.client.calls).toContain('takeResult');
		expect(h.store.window('main-1')!.tabs.map((tab) => tab.id)).toContain(h.ids.a);
	});

	it('says aloud, in the window that took the payload, what merged where', async () => {
		const h = await setup();
		const targetApi = new FakeTabsApi(h.store, 'main-2');
		const said: string[] = [];
		const handoff = createTearHandoff({
			client: h.client,
			api: targetApi,
			flush: async () => {},
			announce: (text) => said.push(text),
		});
		const stop = handoff.connect();
		const payload: TearPayload = {
			v: 1,
			mode: 'tabs',
			what: { kind: 'tabs', value: [h.ids.a!] },
			tabs: [h.ids.a!],
			name: 'a',
			source: { window: 'main-1', index: 0 },
		};
		h.client.fireDropped({ window: 'main-2', payload });
		await settle();
		stop();
		expect(said).toHaveLength(1);
		expect(said[0]).toMatch(/^Merged a into /);
		// A drop from somewhere else carries no tear payload and is not announced.
		said.length = 0;
		handoff.connect();
		h.client.fireDropped({ window: 'main-2', payload: { hello: 1 } });
		await settle();
		expect(said).toEqual([]);
	});
});

describe('where the compositor cannot drag a window', () => {
	it('keeps the in-page card when only the toplevel flag is off', async () => {
		const h = await setup();
		h.client.featureSet = { ...h.client.featureSet, toplevelDrag: false };
		const source = h.source({ tab: h.ids.a! });
		h.hook.update({ x: 400, y: 300 }, source);
		expect(h.card.getState().payload).toEqual({ title: 'a' });
		await settle();
		expect(h.client.toplevelBegins).toEqual([]);
		expect(h.moveTabs).not.toHaveBeenCalled();
	});
});
