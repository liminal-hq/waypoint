// Verifies the in-memory session store models the Shelf window the way the Rust store does
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { FakeTabsApi } from './fakeTabsApi';
import { FakeTabsStore, SHELF_LABEL } from './fakeTabsStore';
import { fileLocation } from './fakeVfsClient';
import { applyTabsEvent } from './tabsApi';

const A = fileLocation('/home/a.txt');

function twoWindows(createWindow?: (label: string) => void) {
	const store = new FakeTabsStore(createWindow ? { createWindow } : {});
	const one = new FakeTabsApi(store, 'main-1');
	const two = new FakeTabsApi(store, 'main-2');
	return { store, one, two };
}

describe('the Shelf window in the in-memory store', () => {
	it('starts docked, and has no snapshot for the Shelf window', async () => {
		const { store, one } = twoWindows();
		expect((await one.getSnapshot()).shelfWindow).toEqual({
			undocked: false,
			geometry: null,
			onTop: false,
		});
		expect(() => store.snapshot(SHELF_LABEL)).toThrow(`no such window: ${SHELF_LABEL}`);
	});

	it('undocking tells every window and the Shelf window, which then reads the shared Shelf', async () => {
		const created = vi.fn();
		const { store, one, two } = twoWindows(created);
		await one.addToShelf([A]);
		const events = new Map<string, string[]>();
		for (const label of ['main-1', 'main-2', SHELF_LABEL]) {
			store.listen(label, (e) => events.set(label, [...(events.get(label) ?? []), e.kind]));
		}
		await one.setShelfUndocked(true);
		expect(created).toHaveBeenCalledWith(SHELF_LABEL, null);
		for (const label of ['main-1', 'main-2', SHELF_LABEL]) {
			expect(events.get(label), label).toEqual(['shelfWindowChanged']);
		}
		expect((await two.getSnapshot()).shelfWindow.undocked).toBe(true);
		const shelf = store.snapshot(SHELF_LABEL);
		expect(shelf.tabs).toEqual([]);
		expect(shelf.shelf.map((item) => item.location.uri)).toEqual([A.uri]);
	});

	it('lets the Shelf window change the Shelf and tells the main windows', async () => {
		const { store, one } = twoWindows();
		await one.setShelfUndocked(true);
		const heard: string[] = [];
		store.listen('main-2', (e) => heard.push(e.kind));
		store.dispatch(SHELF_LABEL, { kind: 'addToShelf', locations: [A], addedMs: 1 });
		expect(heard).toEqual(['shelfChanged']);
		expect(store.violations()).toEqual([]);
	});

	it('refuses the Shelf window while the Shelf is docked', () => {
		const { store } = twoWindows();
		expect(() =>
			store.dispatch(SHELF_LABEL, { kind: 'addToShelf', locations: [A], addedMs: 1 }),
		).toThrow('no such window: shelf');
	});

	it('keeps the on-top choice and the geometry across docking, and saves a move without an event', async () => {
		const { store, one } = twoWindows();
		await one.setShelfUndocked(true);
		await new FakeTabsApi(store, SHELF_LABEL).setShelfOnTop(true);
		const heard: string[] = [];
		store.listen('main-1', (e) => heard.push(e.kind));
		const geometry = { x: 1, y: 2, width: 600, height: 200, maximised: false };
		const revision = store.revision;
		store.dispatch(SHELF_LABEL, { kind: 'setShelfGeometry', geometry });
		expect(heard).toEqual([]);
		expect(store.revision).toBe(revision);
		store.destroyWindow(SHELF_LABEL);
		const snapshot = await one.getSnapshot();
		expect(snapshot.shelfWindow).toEqual({ undocked: false, geometry, onTop: true });
	});

	it('docks again when the Shelf window is destroyed, and a second destroy changes nothing', async () => {
		const { store, one } = twoWindows();
		await one.setShelfUndocked(true);
		store.destroyWindow(SHELF_LABEL);
		expect((await one.getSnapshot()).shelfWindow.undocked).toBe(false);
		const revision = store.revision;
		store.destroyWindow(SHELF_LABEL);
		expect(store.revision).toBe(revision);
	});

	it('undoes the change when the window cannot be made', async () => {
		const { one } = twoWindows(() => {
			throw new Error('no windows here');
		});
		await expect(one.setShelfUndocked(true)).rejects.toMatch('could not create the window');
		expect((await one.getSnapshot()).shelfWindow.undocked).toBe(false);
	});

	it('a window mirrors it by events alone', async () => {
		const { store, one } = twoWindows();
		let mirror = await one.getSnapshot();
		store.listen('main-1', (e) => (mirror = applyTabsEvent(mirror, e)));
		await one.setShelfUndocked(true);
		await new FakeTabsApi(store, SHELF_LABEL).setShelfOnTop(true);
		expect(mirror.shelfWindow).toEqual({ undocked: true, geometry: null, onTop: true });
		expect(mirror.shelfWindow).toEqual((await one.getSnapshot()).shelfWindow);
	});
});
