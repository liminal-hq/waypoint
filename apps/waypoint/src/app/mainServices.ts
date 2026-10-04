// Starts the services the Main window runs on: the real plugins, or in-memory demo folders in dev
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ViewPrefs } from '@liminal-hq/waypoint-protocol/generated/ViewPrefs';
import type { OpenWithClient } from '../openWith/openWithClient';
import type { AppInfoClient } from '../services/appInfoClient';
import type { FolderViewsClient } from '../services/folderViewsClient';
import type { GitClient } from '../services/gitClient';
import type { FakeVfsClient } from '../services/fakeVfsClient';
import type { OpsClient } from '../services/opsClient';
import type { NativeDndClient } from '../services/nativeDndClient';
import type { OsClipboardClient } from '../services/osClipboardClient';
import type { DirScanClient } from '../services/dirScanClient';
import type { DetailsClient } from '../services/detailsClient';
import type { PropertiesWindowClient } from '../services/propertiesWindowClient';
import type { PlacesClient } from '../services/placesClient';
import type { TabsApi } from '../services/tabsApi';
import type { TearoffClient } from '../services/tearoffClient';
import type { TimeFormatClient } from '../services/timeFormatClient';
import type { DevicesClient } from '../devices/devicesClient';
import type { ConnectionsClient } from '../connections/connectionsClient';
import type { ThumbnailsClient } from '../thumbnails/thumbnailsClient';
import type { TrashClient } from '../trash/trashClient';
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
	/** The system file clipboard, kept level with Cut and Copy; without it the clipboard is Waypoint's own. */
	osClipboard?: OsClipboardClient;
	/** The native drag and drop plugin: drops from other applications and drags out of the window; without it drags stay in the page. */
	nativeDnd?: NativeDndClient;
	/** The Trash's state and jobs; without it the Trash place shows no count and offers no actions. */
	trash?: TrashClient;
	/** The drives and volumes behind the sidebar's Devices section; without it the section is not shown. */
	devices?: DevicesClient;
	/** The saved connections and the state of every server login; without it there is no Network section and no Connect dialog. */
	connections?: ConnectionsClient;
	/** Open With: the default and other applications for a file; without it Open With is not offered. */
	openWith?: OpenWithClient;
	/** Thumbnails for the views; without it every view keeps its icons. */
	thumbnails?: ThumbnailsClient;
	/** Entry details, text heads and preview addresses, for Quick Look; without it Space does nothing. */
	details?: DetailsClient;
	/** Properties windows; without it Alt+Enter, the item menu and the Inspector offer none. */
	propertiesWindow?: PropertiesWindowClient;
	/** The directory-size scan behind Overview's biggest folders in Home; without it Overview offers no measurement. */
	dirScan?: DirScanClient;
	/** The application's own details, which About shows; without it About cannot give the version. */
	appInfo?: AppInfoClient;
	/** What each folder remembers about its view, sort and grouping; without it every folder shows the window's. */
	folderViews?: FolderViewsClient;
	/** What Git says about the folders shown: the branch, the Git column and the sidebar's badges; without it the window shows no Git. */
	git?: GitClient;
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
	createOsClipboardClient?(): OsClipboardClient;
	createNativeDndClient?(): NativeDndClient;
	createTrashClient?(): TrashClient;
	createDevicesClient?(): DevicesClient;
	createConnectionsClient?(): ConnectionsClient;
	createOpenWithClient?(): OpenWithClient;
	createThumbnailsClient?(): ThumbnailsClient;
	createDetailsClient?(): DetailsClient;
	createPropertiesWindowClient?(): PropertiesWindowClient;
	createDirScanClient?(): DirScanClient;
	createAppInfoClient?(): AppInfoClient;
	createFolderViewsClient?(): FolderViewsClient;
	createGitClient?(): GitClient;
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
		osClipboard: deps.createOsClipboardClient?.(),
		nativeDnd: deps.createNativeDndClient?.(),
		trash: deps.createTrashClient?.(),
		devices: deps.createDevicesClient?.(),
		connections: deps.createConnectionsClient?.(),
		openWith: deps.createOpenWithClient?.(),
		thumbnails: deps.createThumbnailsClient?.(),
		details: deps.createDetailsClient?.(),
		propertiesWindow: deps.createPropertiesWindowClient?.(),
		dirScan: deps.createDirScanClient?.(),
		appInfo: deps.createAppInfoClient?.(),
		folderViews: deps.createFolderViewsClient?.(),
		git: deps.createGitClient?.(),
		home,
	};
}
