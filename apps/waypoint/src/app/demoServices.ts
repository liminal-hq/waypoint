// The in-memory demo folders the Main window can run on in development, for trying live updates
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fileLocation } from '../services/fakeVfsClient';
import { createDemoClient, DEMO_HOME } from '../browse/demoClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { FakeTimeFormatClient } from '../services/fakeTimeFormatClient';
import { fakeVolume, FakeDevicesClient } from '../devices/fakeDevicesClient';
import { FakeTrashClient } from '../trash/fakeTrashClient';
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
	return {
		client,
		placesClient,
		tabsApi,
		home: DEMO_HOME,
		timeFormat: new FakeTimeFormatClient('h23'),
		ops: createFakeOpsClient(),
		trash: new FakeTrashClient({ count: 3 }),
		devices: new FakeDevicesClient([
			fakeVolume('system', {
				label: 'System',
				kind: 'internal',
				isSystem: true,
				canEject: false,
				canUnmount: false,
				mountPoint: '/',
				total: 500_000_000_000,
				free: 60_000_000_000,
			}),
			fakeVolume('stick', { label: 'USB stick' }),
		]),
		demo: { client },
	};
}
