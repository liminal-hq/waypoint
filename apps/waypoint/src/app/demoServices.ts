// The in-memory demo folders the Main window can run on in development, for trying live updates
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createDemoClient, DEMO_HOME } from '../browse/demoClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import type { MainServices } from './mainServices';

/** Synthetic folders and tabs over them, all in memory. Imported only when `?demo` is in the URL. */
export async function startDemoServices(): Promise<MainServices> {
	const client = createDemoClient();
	const tabsApi = new FakeTabsApi();
	await tabsApi.openTab(DEMO_HOME);
	return { client, tabsApi, home: DEMO_HOME, demo: { client } };
}
