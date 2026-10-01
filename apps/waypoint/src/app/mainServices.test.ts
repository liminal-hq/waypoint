// Tests for starting the Main window's services: the first tab is opened once, and only when needed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakePlacesClient } from '../services/fakePlacesClient';
import { fileLocation, FakeVfsClient } from '../services/fakeVfsClient';
import { startMainServices } from './mainServices';

const home = fileLocation('/home/scott');

function deps(overrides: Partial<Parameters<typeof startMainServices>[0]> = {}) {
	const client = new FakeVfsClient();
	const placesClient = new FakePlacesClient();
	return {
		client,
		placesClient,
		tabsApi: new FakeTabsApi(),
		getHome: vi.fn(async () => home),
		createClient: vi.fn(() => client),
		createPlacesClient: vi.fn(() => placesClient),
		...overrides,
	};
}

describe('startMainServices', () => {
	it('opens the first tab at the home folder when the session is empty', async () => {
		const d = deps();
		const services = await startMainServices(d);
		expect(services.home).toEqual(home);
		expect(services.client).toBe(d.client);
		expect(services.placesClient).toBe(d.placesClient);
		const snapshot = await d.tabsApi.getSnapshot();
		expect(snapshot.tabs).toHaveLength(1);
		expect(snapshot.tabs[0]!.location).toEqual(home);
	});

	it('keeps the tabs a window already has instead of opening another', async () => {
		const d = deps();
		await d.tabsApi.openTab(fileLocation('/tmp'));
		await startMainServices(d);
		const snapshot = await d.tabsApi.getSnapshot();
		expect(snapshot.tabs).toHaveLength(1);
		expect(snapshot.tabs[0]!.location).toEqual(fileLocation('/tmp'));
	});

	it('rejects, without opening a tab, when the home folder cannot be read', async () => {
		const d = deps({ getHome: vi.fn(async () => Promise.reject(new Error('no home'))) });
		await expect(startMainServices(d)).rejects.toThrow('no home');
		expect((await d.tabsApi.getSnapshot()).tabs).toHaveLength(0);
	});
});
