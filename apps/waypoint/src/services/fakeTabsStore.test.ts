// Tests the shared fake store: several windows, hand-off, rollback and the mirror of events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { FakeTabsApi } from './fakeTabsApi';
import { FakeTabsStore } from './fakeTabsStore';
import { fileLocation } from './fakeVfsClient';
import { applyTabsEvent } from './tabsApi';

const loc = (name: string) => fileLocation(`/${name}`);

function twoWindows(options: ConstructorParameters<typeof FakeTabsStore>[0] = {}) {
	const store = new FakeTabsStore(options);
	const one = new FakeTabsApi(store, 'main-1');
	const two = new FakeTabsApi(store, 'main-2');
	const heard = { one: [] as SessionEvent[], two: [] as SessionEvent[] };
	one.onEvent((e) => heard.one.push(e));
	two.onEvent((e) => heard.two.push(e));
	return { store, one, two, heard };
}

describe('FakeTabsStore with several windows', () => {
	it('numbers tabs globally and sends each window only its own events, with gaps in revisions', async () => {
		const { one, two, heard } = twoWindows();
		const a = await one.openTab(loc('a'));
		const b = await two.openTab(loc('b'));
		const c = await one.openTab(loc('c'));
		expect([a, b, c]).toEqual([1, 2, 3]);
		expect(heard.one.flatMap((e) => (e.kind === 'tabOpened' ? [e.tab.id] : []))).toEqual([1, 3]);
		expect(heard.two.flatMap((e) => (e.kind === 'tabOpened' ? [e.tab.id] : []))).toEqual([2]);
		const revisions = heard.one.map((e) => e.revision);
		expect(revisions.every((r, i) => i === 0 || r > (revisions[i - 1] ?? 0))).toBe(true);
		expect(revisions).not.toEqual(revisions.map((_, i) => i + 1));
	});

	it('hands tabs to another window with their ids, history and marks, and the source hears them leave', async () => {
		const { one, two, heard } = twoWindows();
		const a = await one.openTab(loc('a'));
		const b = await one.openTab(loc('b'), { activate: false });
		await one.navigate(b, loc('b/x'));
		await one.setTabColour(b, 'teal');
		await one.pinTab(b, true);
		heard.one.length = 0;

		const label = await one.moveTabs(
			{ kind: 'tabs', value: [b] },
			{ kind: 'existingWindow', label: 'main-2', index: 0 },
		);
		expect(label).toBe('main-2');
		expect(heard.one.map((e) => e.kind)).toContain('tabClosed');
		const moved = (await two.getSnapshot()).tabs[0];
		expect(moved).toMatchObject({ id: b, pinned: true, colour: 'teal' });
		expect(moved?.back.map((l) => l.display)).toEqual(['/b']);
		expect((await one.getSnapshot()).tabs.map((t) => t.id)).toEqual([a]);
	});

	it('creates the window a hand-off needs through the hook, and undoes the change when it fails', async () => {
		const made: [string, number | undefined][] = [];
		let fail = false;
		const store = new FakeTabsStore({
			createWindow: (label, geometry) => {
				if (fail) throw 'creating windows is not available yet';
				made.push([label, geometry?.width]);
			},
		});
		const one = new FakeTabsApi(store, 'main-1');
		const heard: SessionEvent[] = [];
		one.onEvent((e) => heard.push(e));
		const a = await one.openTab(loc('a'));
		await one.openTab(loc('b'));
		heard.length = 0;
		const before = store.toSnapshot();

		fail = true;
		await expect(
			one.moveTabs(
				{ kind: 'tabs', value: [a] },
				{ kind: 'newWindow', label: null, geometry: null },
			),
		).rejects.toBe('could not create the window: creating windows is not available yet');
		await expect(one.openWindow(loc('c'))).rejects.toMatch(/could not create the window/);
		expect(store.toSnapshot()).toEqual(before);
		expect(heard).toEqual([]);

		fail = false;
		const label = await one.moveTabs(
			{ kind: 'tabs', value: [a] },
			{
				kind: 'newWindow',
				label: null,
				geometry: { x: null, y: null, width: 800, height: 600, maximised: false },
			},
		);
		expect(label).toBe('main-2');
		expect(made).toEqual([['main-2', 800]]);
		const target = new FakeTabsApi(store, label);
		expect((await target.getSnapshot()).tabs.map((t) => t.id)).toEqual([a]);
	});

	it("does not let a caller choose a new window's label", async () => {
		const { one } = twoWindows();
		const a = await one.openTab(loc('a'));
		await expect(
			one.moveTabs(
				{ kind: 'tabs', value: [a] },
				{ kind: 'newWindow', label: 'main-9', geometry: null },
			),
		).rejects.toMatch(/cannot be given the label/);
	});

	it('closes a window that loses its last tab under the app policy, and keeps it under the milestone 2 one', async () => {
		let last = 0;
		const app = new FakeTabsStore({ onLastWindowClosed: () => (last += 1) });
		const one = new FakeTabsApi(app, 'main-1');
		const a = await one.openTab(loc('a'));
		await one.closeTab(a);
		expect(app.windowLabels()).toEqual([]);
		expect(last).toBe(1);

		const single = new FakeTabsApi();
		const b = await single.openTab(loc('b'));
		await single.closeTab(b);
		expect(await single.getSnapshot()).toMatchObject({ tabs: [], active: null });
	});

	it('remembers closed tabs and reopens the newest, or a named one, or nothing', async () => {
		const api = new FakeTabsApi();
		expect(await api.reopenTab()).toBeNull();
		const a = await api.openTab(loc('a'));
		const b = await api.openTab(loc('b'));
		const c = await api.openTab(loc('c'));
		await api.closeTab(b);
		await api.closeTab(c);
		expect((await api.getSnapshot()).closed.map((x) => x.tab.id)).toEqual([c, b]);
		expect(await api.reopenTab(b)).toBe(b);
		expect(await api.reopenTab()).toBe(c);
		const snapshot = await api.getSnapshot();
		// Each came back to the index it was closed at, so c (closed at 1) landed before b.
		expect(snapshot.tabs.map((t) => t.id)).toEqual([a, c, b]);
		expect(snapshot.mru).toEqual([c, b]);
		await expect(api.reopenTab(99)).rejects.toBe('no such tab: 99');
	});

	it("returns the ids the plugin returns and rejects with Rust's messages", async () => {
		const api = new FakeTabsApi();
		const a = await api.openTab(loc('a'));
		const b = await api.openTab(loc('b'));
		expect(await api.createGroup([a])).toBe(1);
		expect((await api.getSnapshot()).groups[0]?.name).toBe('Group 1');
		expect(await api.duplicateGroup(1)).toBe(2);
		expect(await api.joinPair([a, b], 'stacked')).toBe(1);
		await expect(api.joinPair([a, b], 'stacked')).rejects.toBe(`tab ${a} is already in a pair`);
		await expect(api.createGroup([])).rejects.toBe(
			'invalid command: a group needs at least one tab',
		);
		await expect(api.renameGroup(42, 'x')).rejects.toBe('no such group: 42');
		await expect(api.swapPanes(42)).rejects.toBe('no such pair: 42');
		await expect(api.closeWindow('main-9')).rejects.toBe('no such window: main-9');
		expect(await api.openWindow(loc('w'))).toBe('main-2');
	});

	it('keeps view and geometry per window and reports them as events', async () => {
		const { one, two, heard } = twoWindows();
		await one.setView({ mode: 'grid', showHidden: true, iconSize: 96 });
		await two.setGeometry({ x: 1, y: 2, width: 640, height: 480, maximised: false });
		expect((await one.getSnapshot()).view.mode).toBe('grid');
		expect((await two.getSnapshot()).view.mode).toBe('list');
		expect((await two.getSnapshot()).geometry?.width).toBe(640);
		expect(heard.one.map((e) => e.kind)).toEqual(['viewChanged']);
		expect(heard.two.map((e) => e.kind)).toEqual(['geometryChanged']);
		// Setting the same view again changes nothing.
		await one.setView({ mode: 'grid', showHidden: true, iconSize: 96 });
		expect(heard.one).toHaveLength(1);
	});
});

describe('applyTabsEvent over every event', () => {
	/** A small deterministic generator, as the Rust fuzz test uses. */
	function generator(seed: number) {
		let state = seed;
		return (n: number) => {
			state = (state * 1664525 + 1013904223) % 4294967296;
			return Math.floor((state / 4294967296) * n);
		};
	}

	it('rebuilds each window from its own events across random commands in two windows', async () => {
		const store = new FakeTabsStore();
		const handles = [new FakeTabsApi(store, 'main-1'), new FakeTabsApi(store, 'main-2')];
		const mirrors: SessionSnapshot[] = [];
		const heard: SessionEvent[][] = [];
		for (const [i, h] of handles.entries()) {
			heard.push([]);
			h.onEvent((e) => heard[i]?.push(e));
			mirrors.push(await h.getSnapshot());
		}
		const next = generator(7);
		const pick = <T>(items: T[]): T | undefined => items[next(items.length)];
		for (let step = 0; step < 400; step += 1) {
			const w = next(2);
			const api = handles[w] as FakeTabsApi;
			if (!store.windowLabels().includes(api.label)) continue;
			const mine = await api.getSnapshot();
			const tab = pick(mine.tabs)?.id ?? 999;
			const group = pick(mine.groups)?.id ?? 999;
			const pair = pick(mine.pairs)?.id ?? 999;
			const other = handles[1 - w] as FakeTabsApi;
			try {
				switch (next(18)) {
					case 0:
						await api.openTab(loc(`t${step}`), {
							after: next(2) ? tab : undefined,
							activate: next(2) === 0,
						});
						break;
					case 1:
						await api.closeTab(tab);
						break;
					case 2:
						await api.activateTab(tab);
						break;
					case 3:
						await api.moveTab(tab, next(6));
						break;
					case 4:
						await api.navigate(tab, loc(`n${step}`));
						break;
					case 5:
						await api.pinTab(tab, next(2) === 0);
						break;
					case 6:
						await api.createGroup([tab, pick(mine.tabs)?.id ?? 999]);
						break;
					case 7:
						await api.addToGroup(tab, group);
						break;
					case 8:
						await api.removeFromGroup(tab);
						break;
					case 9:
						await api.joinPair([tab, pick(mine.tabs)?.id ?? 999], 'sideBySide');
						break;
					case 10:
						await api.toggleSplit(tab);
						break;
					case 11:
						await api.swapPanes(pair);
						break;
					case 12:
						await api.sortGroup(group, 'name');
						break;
					case 13:
						await api.moveGroup(group, next(5));
						break;
					case 14:
						await api.reopenTab();
						break;
					case 15:
						await api.moveTabs({ kind: next(3) === 0 ? 'group' : 'tabs', value: [tab] } as never, {
							kind: 'existingWindow',
							label: other.label,
							index: next(4),
						});
						break;
					case 16:
						await api.duplicateGroup(group);
						break;
					default:
						await api.closeGroup(group);
				}
			} catch (error) {
				// A command on an id that is not there is a normal rejection; anything else is a bug.
				expect(typeof error, `step ${step}`).toBe('string');
			}
			expect(store.violations(), `step ${step}`).toEqual([]);
			for (const [i, h] of handles.entries()) {
				if (!store.windowLabels().includes(h.label)) continue;
				let mirror = mirrors[i] as SessionSnapshot;
				for (const event of (heard[i] as SessionEvent[]).splice(0))
					mirror = applyTabsEvent(mirror, event);
				mirrors[i] = mirror;
				const snapshot = await h.getSnapshot();
				expect({ ...mirror, revision: 0, closed: [] }, `step ${step} ${h.label}`).toEqual({
					...snapshot,
					revision: 0,
					closed: [],
				});
			}
		}
	});
});
