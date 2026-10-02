// Tests the Shelf side of FakeTabsApi: the origin rule, the cap as a typed refusal, and events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import { FakeTabsApi } from './fakeTabsApi';
import { SHELF_LIMIT, originOf } from './fakeTabsStore';
import { fileLocation } from './fakeVfsClient';
import { applyTabsEvent, isShelfFull, shelfFullLimit } from './tabsApi';

describe('originOf', () => {
	it('is the containing folder, and a root is its own origin', () => {
		expect(originOf(fileLocation('/home/a/b.txt'))).toEqual(fileLocation('/home/a'));
		expect(originOf(fileLocation('/b.txt'))).toEqual({ display: '/', uri: 'file:///' });
		const root = { display: '/', uri: 'file:///' };
		expect(originOf(root)).toBe(root);
	});

	it('knows Windows drives and remote hosts', () => {
		expect(originOf({ display: 'C:\\Users\\a.txt', uri: 'file:///C:/Users/a.txt' })).toEqual({
			display: 'C:\\Users',
			uri: 'file:///C:/Users',
		});
		expect(originOf({ display: 'C:\\a.txt', uri: 'file:///C:/a.txt' })).toEqual({
			display: 'C:\\',
			uri: 'file:///C:/',
		});
		expect(originOf({ display: 'host/d/f', uri: 'sftp://host/d/f' }).uri).toBe('sftp://host/d');
		const host = { display: 'host', uri: 'sftp://host' };
		expect(originOf(host)).toBe(host);
	});
});

describe('the Shelf on FakeTabsApi', () => {
	it('adds, removes and clears with one event each, whole', async () => {
		const api = new FakeTabsApi();
		const events: SessionEvent[] = [];
		api.onEvent((e) => events.push(e));
		await api.addToShelf([fileLocation('/d/a'), fileLocation('/d/b'), fileLocation('/d/a')]);
		let snapshot = await api.getSnapshot();
		expect(snapshot.shelf.map((i) => [i.id, i.name, i.origin.display])).toEqual([
			[1, 'a', '/d'],
			[2, 'b', '/d'],
		]);
		await api.addToShelf([fileLocation('/d/a')]);
		await api.removeFromShelf([1, 99]);
		await api.clearShelf();
		await api.clearShelf();
		expect(events.map((e) => e.kind)).toEqual(['shelfChanged', 'shelfChanged', 'shelfChanged']);
		let mirror: SessionSnapshot = { ...snapshot, shelf: [], revision: 0 };
		for (const e of events) mirror = applyTabsEvent(mirror, e);
		snapshot = await api.getSnapshot();
		expect(mirror.shelf).toEqual(snapshot.shelf);
		expect(snapshot.shelf).toEqual([]);
	});

	it('refuses past the cap with a typed message and adds nothing', async () => {
		const api = new FakeTabsApi();
		const many = Array.from({ length: SHELF_LIMIT }, (_, i) => fileLocation(`/d/f${i}`));
		await api.addToShelf(many);
		const error = await api.addToShelf([fileLocation('/d/extra')]).catch((e: unknown) => e);
		expect(isShelfFull(error)).toBe(true);
		expect(shelfFullLimit(error)).toBe(SHELF_LIMIT);
		expect((await api.getSnapshot()).shelf).toHaveLength(SHELF_LIMIT);
	});

	it('reads the rejection of the real plugin too', () => {
		expect(shelfFullLimit({ kind: 'shelfFull', message: 'x', limit: 500 })).toBe(500);
		expect(shelfFullLimit({ kind: 'session', message: 'x' })).toBeNull();
		expect(shelfFullLimit(new Error('nope'))).toBeNull();
	});
});
