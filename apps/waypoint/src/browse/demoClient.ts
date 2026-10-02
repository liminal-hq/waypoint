// A synthetic home folder for the Main window until the real file system client replaces it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import {
	FakeVfsClient,
	fileLocation,
	makeEntry,
	syntheticEntries,
} from '../services/fakeVfsClient';

export const DEMO_HOME: Location = fileLocation('/home/demo');

/** The demo's Trash, which lists a few items that came from the demo home. */
export const DEMO_TRASH: Location = { display: 'Trash', uri: 'trash:/' };

/** What the demo Trash holds: a file, a folder and a picture, trashed on different days. */
export function demoTrashEntries(): Entry[] {
	const day = 86_400_000;
	const now = 1_700_000_000_000;
	const item = (id: number, name: string, daysAgo: number, overrides: Partial<Entry> = {}) =>
		makeEntry(id, name, {
			modifiedMs: null,
			originalPath: '/home/demo/Documents',
			deletedMs: now - daysAgo * day,
			...overrides,
		});
	return [
		item(1, 'budget-2025.pdf', 1),
		item(2, 'old drafts', 12, { kind: 'directory', size: 48_200 }),
		item(3, 'holiday.jpg', 40, { originalPath: '/home/demo/Pictures' }),
	];
}

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
	client.markTrash(DEMO_TRASH);
	client.setFolder(DEMO_TRASH, demoTrashEntries());
	client.setFolder(fileLocation('/home'), [makeEntry(1, 'demo', { kind: 'directory' })]);
	client.setFolder(fileLocation('/'), [makeEntry(1, 'home', { kind: 'directory' })]);
	return client;
}
