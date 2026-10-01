// Starts the services the Main window runs on: the real plugins, or in-memory demo folders in dev
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { FakeVfsClient } from '../services/fakeVfsClient';
import type { PlacesClient } from '../services/placesClient';
import type { TabsApi } from '../services/tabsApi';
import type { VfsClient } from '../services/vfsClient';

export interface MainServices {
	client: VfsClient;
	placesClient: PlacesClient;
	tabsApi: TabsApi;
	/** Where a new tab opens when nothing says otherwise. */
	home: Location;
	/** Set only for the in-memory demo, whose folders the dev controls can change. */
	demo?: { client: FakeVfsClient };
}

export interface MainServicesDeps {
	getHome(): Promise<Location>;
	tabsApi: TabsApi;
	createClient(): VfsClient;
	createPlacesClient(): PlacesClient;
}

/**
 * Reads the home folder, makes sure the window's session has a first tab, and returns the clients.
 * A window that already has tabs (a reload, a restored session) keeps them: only an empty session
 * opens one.
 */
export async function startMainServices(deps: MainServicesDeps): Promise<MainServices> {
	const home = await deps.getHome();
	const snapshot = await deps.tabsApi.getSnapshot();
	if (snapshot.tabs.length === 0) await deps.tabsApi.openTab(home);
	return {
		client: deps.createClient(),
		placesClient: deps.createPlacesClient(),
		tabsApi: deps.tabsApi,
		home,
	};
}
