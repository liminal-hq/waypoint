// Shared setup for tests that drive the whole browsing area: a small folder tree, tabs and providers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent, render } from '@testing-library/react';
import { Workspace } from '../app/Workspace';
import { FolderViewsProvider } from '../browse/FolderViewsContext';
import { GitProvider } from '../git/GitContext';
import type { GitClient } from '../services/gitClient';
import type { FolderViewsClient } from '../services/folderViewsClient';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import type { NativeDndClient } from '../services/nativeDndClient';
import type { ShelfWindowClient } from '../services/shelfWindowClient';
import type { TearoffClient } from '../services/tearoffClient';
import { PlacesClientProvider } from '../sidebar/PlacesClientContext';
import { OpsProvider } from '../ops/OpsContext';
import { OpsResolverHost } from '../ops/OpsResolverHost';
import type { OpsClient } from '../services/opsClient';
import type { SettingsClient } from '../services/settingsClient';
import { SettingsProvider } from '../settings/SettingsContext';
import { TabsProvider } from '../tabs/TabsContext';
import { DevicesClientProvider } from '../devices/DevicesClientContext';
import type { DevicesClient } from '../devices/devicesClient';
import type { ArchiveClient } from '../archives/archiveClient';
import { ArchiveClientProvider } from '../archives/ArchiveContext';
import { ConnectionsProvider } from '../connections/ConnectionsContext';
import type { ConnectionsClient } from '../connections/connectionsClient';
import type { TrashClient } from '../trash/trashClient';
import { TrashClientProvider } from '../trash/TrashClientContext';
import type { ThumbnailsClient } from '../thumbnails/thumbnailsClient';
import { ThumbnailsProvider } from '../thumbnails/ThumbnailsContext';
import { DetailsClientProvider } from '../inspector/DetailsClientContext';
import type { DetailsClient } from '../services/detailsClient';
import { PropertiesWindowProvider } from '../inspector/PropertiesWindowContext';
import type { PropertiesWindowClient } from '../services/propertiesWindowClient';
import type { DirScanClient } from '../services/dirScanClient';
import { HomeScanProvider } from '../overview/HomeScanContext';

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

/** Renders the browsing area over `client` with one tab open at `HOME` (and the sidebar hidden unless `options.sidebar`; `options.tearoff` is the tear-off plugin, `options.trash` the Trash service, `options.ops` the operations queue). */
export async function renderWorkspace(
	client: FakeVfsClient = createTree(),
	tabs: FakeTabsApi = new FakeTabsApi(),
	places: FakePlacesClient = new FakePlacesClient({ places: fakePlaces('/home/test') }),
	options: {
		sidebar?: boolean;
		tearoff?: TearoffClient;
		trash?: TrashClient;
		/** The volumes service behind the sidebar's Devices section. */
		devices?: DevicesClient;
		/** The saved connections and login states (no Network section or Connect dialog when omitted). */
		connections?: ConnectionsClient;
		/** The operations queue: the window follows it and answers the jobs it started (`main-1`). */
		ops?: OpsClient;
		/** The settings the window follows (the defaults when omitted). */
		settings?: SettingsClient;
		/** The native drag and drop plugin (drops from other applications, drags out of the window). */
		nativeDnd?: NativeDndClient;
		/** The Shelf window's raise and hide, while the Shelf is undocked. */
		shelfWindow?: ShelfWindowClient;
		/** What each folder remembers about its view (every folder shows the window's when omitted). */
		folderViews?: FolderViewsClient;
		/** What Git says about the folders shown (the window shows no Git when omitted). */
		git?: GitClient;
		/** The thumbnails service (the views keep their icons when omitted). */
		thumbnails?: ThumbnailsClient;
		/** Entry details, folder sizes and previews for the Inspector. */
		details?: DetailsClient;
		/** The Properties windows service (Alt+Enter and the item menu offer none when omitted). */
		propertiesWindow?: PropertiesWindowClient;
		/** The archive provider's password command and slow-listing notice. */
		archives?: ArchiveClient;
		/** The directory-size scan behind Overview's biggest folders in Home (Overview offers no measurement when omitted). */
		dirScan?: DirScanClient;
	} = {},
) {
	if ((await tabs.getSnapshot()).tabs.length === 0) await tabs.openTab(HOME);
	const tabbed = (
		<TabsProvider api={tabs} home={HOME}>
			<FolderViewsProvider client={options.folderViews}>
				<GitProvider client={options.git}>
					<Workspace
						tearoff={options.tearoff}
						nativeDnd={options.nativeDnd}
						shelfWindow={options.shelfWindow}
					/>
				</GitProvider>
			</FolderViewsProvider>
		</TabsProvider>
	);
	const thumbed = options.thumbnails ? (
		<ThumbnailsProvider client={options.thumbnails}>{tabbed}</ThumbnailsProvider>
	) : (
		tabbed
	);
	const detailed = options.details ? (
		<DetailsClientProvider client={options.details}>{thumbed}</DetailsClientProvider>
	) : (
		thumbed
	);
	const windowed = options.propertiesWindow ? (
		<PropertiesWindowProvider client={options.propertiesWindow}>
			{detailed}
		</PropertiesWindowProvider>
	) : (
		detailed
	);
	const workspace = options.settings ? (
		<SettingsProvider client={options.settings}>{windowed}</SettingsProvider>
	) : (
		windowed
	);
	const view = render(
		<VfsClientProvider client={client}>
			<PlacesClientProvider client={places}>
				<TrashClientProvider client={options.trash}>
					<DevicesClientProvider client={options.devices}>
						<ArchiveClientProvider client={options.archives ?? null}>
							<ConnectionsProvider client={options.connections}>
								<HomeScanProvider client={options.dirScan}>
									{options.ops ? (
										<OpsProvider client={options.ops} windowLabel="main-1">
											{workspace}
											<OpsResolverHost />
										</OpsProvider>
									) : (
										workspace
									)}
								</HomeScanProvider>
							</ConnectionsProvider>
						</ArchiveClientProvider>
					</DevicesClientProvider>
				</TrashClientProvider>
			</PlacesClientProvider>
		</VfsClientProvider>,
	);
	// The sidebar's folder tree holds listings of its own, which tests of the tabs' listings would
	// have to count around, so it starts hidden unless a test asks for it.
	if (!options.sidebar) fireEvent.keyDown(window, { key: 'F9' });
	return { client, tabs, places, ...view };
}
