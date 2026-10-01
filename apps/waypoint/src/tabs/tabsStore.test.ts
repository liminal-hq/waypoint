// Verifies the tab store mirrors the session: the snapshot, events before it and replays
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { describe, expect, it } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { fileLocation } from '../services/fakeVfsClient';
import type { TabsApi } from '../services/tabsApi';
import { createTabsStore } from './tabsStore';

const A = fileLocation('/a');
const B = fileLocation('/b');
const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('createTabsStore', () => {
	it('starts empty, then holds the session snapshot', async () => {
		const api = new FakeTabsApi();
		await api.openTab(A);
		const { store, dispose } = createTabsStore(api);
		expect(store.getState().snapshot).toBeNull();
		await flush();
		expect(store.getState().snapshot?.tabs.map((t) => t.location.uri)).toEqual([A.uri]);
		dispose();
	});

	it('follows events, including navigation with its history', async () => {
		const api = new FakeTabsApi();
		const { store, dispose } = createTabsStore(api);
		await flush();
		const id = await api.openTab(A);
		await api.navigate(id, B);
		const tab = store.getState().snapshot!.tabs[0]!;
		expect(tab.location.uri).toBe(B.uri);
		expect(tab.back.map((l) => l.uri)).toEqual([A.uri]);
		dispose();
	});

	it('applies events that arrive before the snapshot, and skips those it already holds', async () => {
		const api = new FakeTabsApi();
		await api.openTab(A);
		const late: { snapshot: () => void } = { snapshot: () => {} };
		const slow: TabsApi = Object.assign(Object.create(api) as TabsApi, {
			getSnapshot: () =>
				new Promise<SessionSnapshot>((resolve) => {
					late.snapshot = () => void api.getSnapshot().then(resolve);
				}),
		});
		const { store, dispose } = createTabsStore(slow);
		await api.openTab(B); // an event before the snapshot is read
		late.snapshot(); // the snapshot already includes it
		await flush();
		expect(store.getState().snapshot?.tabs).toHaveLength(2);
		expect(store.getState().snapshot?.active).toBe(2);
		dispose();
	});

	it('stops following after dispose', async () => {
		const api = new FakeTabsApi();
		const { store, dispose } = createTabsStore(api);
		await flush();
		dispose();
		await api.openTab(A);
		expect(store.getState().snapshot?.tabs).toEqual([]);
	});
});
