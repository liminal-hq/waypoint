// A synthetic home folder for the Main window until the real file system client replaces it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import {
	FakeVfsClient,
	fileLocation,
	makeEntry,
	syntheticEntries,
} from '../services/fakeVfsClient';

export const DEMO_HOME: Location = fileLocation('/home/demo');

/**
 * A fake client serving a home folder of a few thousand mixed entries (with a little latency so
 * loading shows), a small listing inside each of its folders, and the two folders above it. Folders
 * nested deeper do not exist, which is what shows the not-found state.
 */
export function createDemoClient(entryCount = 5000): FakeVfsClient {
	const client = new FakeVfsClient({ latencyMs: 40 });
	const home = syntheticEntries(entryCount);
	client.setFolder(DEMO_HOME, home);
	for (const entry of home) {
		if (entry.kind === 'directory') {
			client.setFolder(fileLocation(`/home/demo/${entry.name}`), syntheticEntries(12));
		}
	}
	client.setFolder(fileLocation('/home'), [makeEntry(1, 'demo', { kind: 'directory' })]);
	client.setFolder(fileLocation('/'), [makeEntry(1, 'home', { kind: 'directory' })]);
	return client;
}
