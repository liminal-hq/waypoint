// A synthetic home folder for the Main window until the real file system client replaces it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { FakeVfsClient, fileLocation, syntheticEntries } from '../services/fakeVfsClient';

export const DEMO_HOME: Location = fileLocation('/home/demo');

/** A fake client serving one folder of a few thousand mixed entries, with a little latency so loading shows. */
export function createDemoClient(entryCount = 5000): FakeVfsClient {
	const client = new FakeVfsClient({ latencyMs: 40 });
	client.setFolder(DEMO_HOME, syntheticEntries(entryCount));
	return client;
}
