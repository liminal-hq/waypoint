// Starts the services the Main window runs on: the real plugins, or in-memory demo folders in dev
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ViewPrefs } from '@liminal-hq/waypoint-protocol/generated/ViewPrefs';
import type { FakeVfsClient } from '../services/fakeVfsClient';
import type { OpsClient } from '../services/opsClient';
import type { PlacesClient } from '../services/placesClient';
import type { TabsApi } from '../services/tabsApi';
import type { TearoffClient } from '../services/tearoffClient';
import type { TimeFormatClient } from '../services/timeFormatClient';
import type { VfsClient } from '../services/vfsClient';

export interface MainServices {
	client: VfsClient;
	placesClient: PlacesClient;
	tabsApi: TabsApi;
	/** Where a new tab opens when nothing says otherwise. */
	home: Location;
	/** The window's saved view choices, applied once as the window starts. */
	view?: ViewPrefs;
	/** A sentence to show once when the last session could not be restored. */
	notice?: string | null;
	/** The tear-off plugin for the new-window phase of a tab drag; without it a release outside the strip does nothing. */
	tearoff?: TearoffClient;
	/** The system's 12/24-hour setting; without it times follow the locale's own convention. */
	timeFormat?: TimeFormatClient;
	/** The operations queue behind the status bar ring; without it the window has no ring. */
	ops?: OpsClient;
	/** Set only for the in-memory demo, whose folders the dev controls can change. */
	demo?: { client: FakeVfsClient };
}

export interface MainServicesDeps {
	getHome(): Promise<Location>;
	tabsApi: TabsApi;
	createClient(): VfsClient;
	createPlacesClient(): PlacesClient;
	createTearoffClient?(): TearoffClient;
	createTimeFormatClient?(): TimeFormatClient;
	createOpsClient?(): OpsClient;
	/** The one-time sentence about a session that could not be restored; `null` when it was. */
	getRestoreNotice?(): Promise<string | null>;
}

/**
 * Reads the home folder, makes sure the window's session has a first tab, and returns the clients.
 * A window that already has tabs (a reload, a restored session) keeps them: only an empty session
 * opens one. The snapshot's view choices travel with the services so the first render uses them.
 */
export async function startMainServices(deps: MainServicesDeps): Promise<MainServices> {
	const home = await deps.getHome();
	const snapshot = await deps.tabsApi.getSnapshot();
	if (snapshot.tabs.length === 0) await deps.tabsApi.openTab(home);
	// Asked of Rust once per run; a failure to ask is not worth a failed start.
	const notice = await (deps.getRestoreNotice?.() ?? Promise.resolve(null)).catch(() => null);
	return {
		view: snapshot.view,
		notice,
		client: deps.createClient(),
		placesClient: deps.createPlacesClient(),
		tabsApi: deps.tabsApi,
		tearoff: deps.createTearoffClient?.(),
		timeFormat: deps.createTimeFormatClient?.(),
		ops: deps.createOpsClient?.(),
		home,
	};
}
