// Tests FakeTabsApi's reducer semantics and the event applier the UI mirrors state with
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { FakeTabsApi } from './fakeTabsApi';
import { fileLocation } from './fakeVfsClient';
import { applyTabsEvent } from './tabsApi';

const firstTab = async (api: FakeTabsApi) => {
	const tab = (await api.getSnapshot()).tabs[0];
	if (!tab) throw new Error('no tab');
	return tab;
};
const order = (snapshot: SessionSnapshot) => snapshot.tabs.map((t) => t.id);

function setup() {
	const api = new FakeTabsApi();
	const events: SessionEvent[] = [];
	api.onEvent((e) => events.push(e));
	return { api, events };
}

describe('FakeTabsApi', () => {
	it('starts empty, which is a valid session', async () => {
		const { api } = setup();
		expect(await api.getSnapshot()).toMatchObject({ revision: 0, tabs: [], active: null });
	});

	it('activates the first tab even when not asked', async () => {
		const { api } = setup();
		const a = await api.openTab(fileLocation('/a'), { activate: false });
		expect((await api.getSnapshot()).active).toBe(a);
	});

	it('opens after a tab and activates or not', async () => {
		const { api } = setup();
		const a = await api.openTab(fileLocation('/a'));
		const b = await api.openTab(fileLocation('/b'), { activate: false });
		const c = await api.openTab(fileLocation('/c'), { after: a });
		const snapshot = await api.getSnapshot();
		expect(order(snapshot)).toEqual([a, c, b]);
		expect(snapshot.active).toBe(c);
	});

	it('closing the active tab activates the tab that takes its place, then the one before', async () => {
		const { api } = setup();
		const a = await api.openTab(fileLocation('/a'));
		const b = await api.openTab(fileLocation('/b'), { activate: false });
		const c = await api.openTab(fileLocation('/c'), { activate: false });
		await api.activateTab(b);
		await api.closeTab(b);
		expect((await api.getSnapshot()).active).toBe(c);
		await api.closeTab(c);
		expect((await api.getSnapshot()).active).toBe(a);
		await api.closeTab(a);
		expect(await api.getSnapshot()).toMatchObject({ tabs: [], active: null });
	});

	it('moves a tab, clamping the index, and ignores a no-op move', async () => {
		const { api, events } = setup();
		const a = await api.openTab(fileLocation('/a'));
		const b = await api.openTab(fileLocation('/b'));
		await api.moveTab(a, 99);
		expect(order(await api.getSnapshot())).toEqual([b, a]);
		const before = events.length;
		await api.moveTab(a, 1);
		expect(events).toHaveLength(before);
	});

	it('keeps history in step through navigate, back and forward', async () => {
		const { api } = setup();
		const t = await api.openTab(fileLocation('/home'));
		for (const p of ['/one', '/two', '/three']) await api.navigate(t, fileLocation(p));
		await api.back(t);
		await api.back(t);
		let tab = await firstTab(api);
		expect(tab.location).toEqual(fileLocation('/one'));
		expect(tab.back).toEqual([fileLocation('/home')]);
		expect(tab.forward).toEqual([fileLocation('/three'), fileLocation('/two')]);
		await api.forward(t);
		await api.navigate(t, fileLocation('/elsewhere'));
		tab = await firstTab(api);
		expect(tab.forward).toEqual([]);
		expect(tab.back.map((l) => l.display)).toEqual(['/home', '/one', '/two']);
	});

	it('makes no events for commands that change nothing', async () => {
		const { api, events } = setup();
		const t = await api.openTab(fileLocation('/a'));
		const before = events.length;
		await api.activateTab(t);
		await api.back(t);
		await api.forward(t);
		await api.navigate(t, fileLocation('/a'));
		expect(events).toHaveLength(before);
	});

	it('rejects an unknown tab and leaves the session alone', async () => {
		const { api } = setup();
		await api.openTab(fileLocation('/a'));
		const before = await api.getSnapshot();
		await expect(api.closeTab(99)).rejects.toBe('no such tab: 99');
		await expect(api.openTab(fileLocation('/x'), { after: 99 })).rejects.toBe('no such tab: 99');
		expect(await api.getSnapshot()).toEqual(before);
	});

	it('stops delivering events after unsubscribe', async () => {
		const api = new FakeTabsApi();
		const heard: SessionEvent[] = [];
		const off = api.onEvent((e) => heard.push(e));
		await api.openTab(fileLocation('/a'));
		off();
		await api.openTab(fileLocation('/b'));
		expect(heard.map((e) => e.kind)).toEqual(['tabOpened', 'tabActivated']);
	});

	it('gives every event the next revision, and applying them rebuilds the snapshot', async () => {
		const { api, events } = setup();
		let mirror = await api.getSnapshot();
		const a = await api.openTab(fileLocation('/a'));
		const b = await api.openTab(fileLocation('/b'), { activate: false });
		await api.navigate(a, fileLocation('/a/x'));
		await api.moveTab(b, 0);
		await api.back(a);
		await api.activateTab(b);
		await api.closeTab(b);
		await api.openTab(fileLocation('/c'), { after: a });
		await api.closeTab(a);
		expect(events.map((e) => e.revision)).toEqual(events.map((_, i) => i + 1));
		for (const event of events) mirror = applyTabsEvent(mirror, event);
		const snapshot = await api.getSnapshot();
		// Closing a tab emits no event for the closed list: a menu reads a fresh snapshot for it.
		expect(snapshot.closed.length).toBe(2);
		expect({ ...mirror, closed: [] }).toEqual({ ...snapshot, closed: [] });
	});

	it('applyTabsEvent ignores an event the snapshot already includes', async () => {
		const { api, events } = setup();
		await api.openTab(fileLocation('/a'));
		const snapshot = await api.getSnapshot();
		expect(applyTabsEvent(snapshot, events[0]!)).toBe(snapshot);
	});
});
