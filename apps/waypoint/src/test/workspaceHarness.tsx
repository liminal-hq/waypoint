// Shared setup for tests that drive the whole browsing area: a small folder tree, tabs and providers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent, render } from '@testing-library/react';
import { Workspace } from '../app/Workspace';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import type { TearoffClient } from '../services/tearoffClient';
import { PlacesClientProvider } from '../sidebar/PlacesClientContext';
import { TabsProvider } from '../tabs/TabsContext';
import type { TrashClient } from '../trash/trashClient';
import { TrashClientProvider } from '../trash/TrashClientContext';

export const HOME = fileLocation('/home/test');
export const DOCS = fileLocation('/home/test/docs');
export const MUSIC = fileLocation('/home/test/music');

/** `/`, `/home`, the home folder with two sub-folders and some files, and the sub-folders. */
export function createTree(): FakeVfsClient {
	const client = new FakeVfsClient();
	client.setFolder(fileLocation('/'), [makeEntry(1, 'home', { kind: 'directory' })]);
	client.setFolder(fileLocation('/home'), [makeEntry(1, 'test', { kind: 'directory' })]);
	client.setFolder(HOME, [
		makeEntry(1, 'docs', { kind: 'directory' }),
		makeEntry(2, 'music', { kind: 'directory' }),
		makeEntry(3, 'notes.txt'),
		makeEntry(4, 'photo.jpg'),
	]);
	client.setFolder(DOCS, [makeEntry(1, 'report.pdf'), makeEntry(2, 'letter.md')]);
	client.setFolder(MUSIC, [makeEntry(1, 'song.mp3')]);
	return client;
}

/** Renders the browsing area over `client` with one tab open at `HOME` (and the sidebar hidden unless `options.sidebar`; `options.tearoff` is the tear-off plugin, `options.trash` the Trash service). */
export async function renderWorkspace(
	client: FakeVfsClient = createTree(),
	tabs: FakeTabsApi = new FakeTabsApi(),
	places: FakePlacesClient = new FakePlacesClient({ places: fakePlaces('/home/test') }),
	options: { sidebar?: boolean; tearoff?: TearoffClient; trash?: TrashClient } = {},
) {
	if ((await tabs.getSnapshot()).tabs.length === 0) await tabs.openTab(HOME);
	const view = render(
		<VfsClientProvider client={client}>
			<PlacesClientProvider client={places}>
				<TrashClientProvider client={options.trash}>
					<TabsProvider api={tabs} home={HOME}>
						<Workspace tearoff={options.tearoff} />
					</TabsProvider>
				</TrashClientProvider>
			</PlacesClientProvider>
		</VfsClientProvider>,
	);
	// The sidebar's folder tree holds listings of its own, which tests of the tabs' listings would
	// have to count around, so it starts hidden unless a test asks for it.
	if (!options.sidebar) fireEvent.keyDown(window, { key: 'F9' });
	return { client, tabs, places, ...view };
}
