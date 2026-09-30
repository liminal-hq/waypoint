// The in-memory demo folders the Main window can run on in development, for trying live updates
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fileLocation } from '../services/fakeVfsClient';
import { createDemoClient, DEMO_HOME } from '../browse/demoClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import type { MainServices } from './mainServices';

/** Synthetic folders and tabs over them, all in memory. Imported only when `?demo` is in the URL. */
export async function startDemoServices(): Promise<MainServices> {
	const client = createDemoClient();
	const tabsApi = new FakeTabsApi();
	await tabsApi.openTab(DEMO_HOME);
	const placesClient = new FakePlacesClient({
		places: fakePlaces('/home/demo'),
		// One folder that exists and one that does not, which shows the not-found state.
		favourites: [
			{ label: 'Demo home', location: DEMO_HOME },
			{ label: 'Deleted folder', location: fileLocation('/home/demo/deleted') },
		],
	});
	return { client, placesClient, tabsApi, home: DEMO_HOME, demo: { client } };
}
