// The browsing area of the Main window: tabs and toolbar over the active tab's file view and status bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { EntryContextMenu } from '../browse/EntryContextMenu';
import { ListingManager } from '../browse/listingManager';
import { BackgroundContextMenu } from '../browse/BackgroundContextMenu';
import { FileView } from '../browse/FileView';
import type { MenuRequest } from '../browse/useListInteractions';
import type { SessionState } from '../browse/useListingSession';
import { useVfsClient } from '../browse/VfsClientContext';
import { useViewShortcuts } from '../browse/useViewShortcuts';
import { followHints, HINT_INTERVAL_MS } from '../browse/tabHints';
import { onFlushHints } from '../services/flushHintsEvent';
import {
	createViewStore,
	followView,
	useViewState,
	viewFromPrefs,
	ViewStoreContext,
	type ViewStore,
} from '../browse/viewStore';
import type { ViewPrefs } from '@liminal-hq/waypoint-protocol/generated/ViewPrefs';
import { tf, t } from '../i18n/messages';
import { Sidebar } from '../sidebar/Sidebar';
import { SidebarToggle } from '../sidebar/SidebarToggle';
import {
	createSidebarStore,
	SidebarStoreContext,
	useSidebarState,
	type SidebarStore,
} from '../sidebar/sidebarStore';
import { usePlacesClient } from '../sidebar/PlacesClientContext';
import { useSidebarShortcuts } from '../sidebar/useSidebarShortcuts';
import { NavigationBar } from '../nav/NavigationBar';
import { useNavigation } from '../nav/useNavigation';
import { useOpenEntry, type EntryAction } from '../nav/useOpenEntry';
import { StatusBar } from '../status/StatusBar';
import { ViewSwitcher } from '../status/ViewSwitcher';
import { TabStrip } from '../tabs/TabStrip';
import { tabDomId, TAB_PANEL_ID } from '../tabs/tabIds';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { onNotice } from '../tabs/notices';
import { useTabShortcuts } from '../tabs/useTabShortcuts';
import { useWindowShortcuts } from '../tabs/windowActions';
import styles from './Workspace.module.css';

const OPENING: SessionState = { status: 'opening' };

/** How long a failure stays in the status bar. */
const NOTICE_MS = 6000;

/** What the window starts from: the saved view, and a sentence to show when the last session could not be restored. */
export interface WorkspaceStartup {
	view?: ViewPrefs;
	notice?: string | null;
}

/** The browsing area. It owns the window's view choices (list or grid, icon size, hidden files). */
export function Workspace({ startup }: { startup?: WorkspaceStartup }) {
	const [viewStore] = useState(() =>
		createViewStore(startup?.view ? viewFromPrefs(startup.view) : {}),
	);
	const [sidebarStore] = useState(() => createSidebarStore());
	return (
		<ViewStoreContext.Provider value={viewStore}>
			<SidebarStoreContext.Provider value={sidebarStore}>
				<WorkspaceBody
					viewStore={viewStore}
					sidebarStore={sidebarStore}
					startupNotice={startup?.notice ?? null}
				/>
			</SidebarStoreContext.Provider>
		</ViewStoreContext.Provider>
	);
}

function WorkspaceBody({
	viewStore,
	sidebarStore,
	startupNotice,
}: {
	viewStore: ViewStore;
	sidebarStore: SidebarStore;
	startupNotice: string | null;
}) {
	const client = useVfsClient();
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const [manager] = useState(
		() =>
			new ListingManager(client, {
				// A new listing keeps the hidden-files choice; the sort is inherited from the
				// listing the tab had before.
				viewMode: () => viewStore.getState().mode,
				openOptions: (inherited) => ({
					...(inherited ? { sort: inherited } : {}),
					filter: { showHidden: viewStore.getState().showHidden },
				}),
			}),
	);
	// Numbered, so the same message arriving again restarts its timer.
	const [notice, setNotice] = useState<{ id: number; text: string } | null>(
		startupNotice ? { id: 0, text: startupNotice } : null,
	);
	const noticeCount = useRef(0);
	const [menu, setMenu] = useState<MenuRequest | null>(null);
	const navigation = useNavigation();
	const onFailure = useCallback(
		(entry: Entry, action: EntryAction) =>
			setNotice({
				id: ++noticeCount.current,
				text:
					action === 'favourite'
						? t('sidebar.favourites.failed')
						: tf(action === 'copyPath' ? 'status.copyPathFailed' : 'status.openFailed', {
								name: entry.name,
							}),
			}),
		[],
	);
	const { open, openInNewTab, copyPath, addToFavourites } = useOpenEntry(navigation, onFailure);
	const places = usePlacesClient();
	// A message from the sidebar, numbered like the others so a repeat restarts the timer.
	const notify = useCallback((text: string) => setNotice({ id: ++noticeCount.current, text }), []);
	const pinCurrent = useCallback(
		(location: Location) => {
			places.addFavourite(location).catch((error: unknown) => {
				console.warn('could not add the favourite', error);
				notify(t('sidebar.favourites.failed'));
			});
		},
		[places, notify],
	);
	useSidebarShortcuts(sidebarStore, navigation.tab?.location, pinCurrent);
	const sidebarOpen = useSidebarState((state) => state.open);
	useTabShortcuts();
	useWindowShortcuts();
	// Messages from code with no route to the status bar (a window that could not open).
	useEffect(() => onNotice(notify), [notify]);
	useViewShortcuts(viewStore);
	const mode = useViewState((view) => view.mode);
	const gridSize = useViewState((view) => view.gridSize);
	const showHidden = useViewState((view) => view.showHidden);

	// One listing per tab, kept in step with the session (A9, A20).
	useEffect(() => {
		if (snapshot) manager.sync(snapshot.tabs, snapshot.active);
	}, [manager, snapshot]);
	useEffect(() => () => manager.dispose(), [manager]);
	// The view choices and the active tab's scroll and focus go to the session so a restart brings
	// them back (the page applies them once, when it starts).
	useEffect(() => followView(viewStore, api), [viewStore, api]);
	const activeId = useRef<number | null>(null);
	activeId.current = snapshot?.active ?? null;
	useEffect(
		() =>
			followHints(
				api,
				() => {
					const tab = activeId.current;
					const state = tab === null ? undefined : manager.stateFor(tab);
					return tab !== null && state?.status === 'ready'
						? { tab, session: state.session, mode: viewStore.getState().mode }
						: null;
				},
				HINT_INTERVAL_MS,
				(tab) => {
					// Any tab with an open listing, so a tab can report just before it leaves for another window.
					const state = manager.stateFor(tab);
					return state?.status === 'ready'
						? { tab, session: state.session, mode: viewStore.getState().mode }
						: null;
				},
				onFlushHints,
			),
		[api, manager, viewStore],
	);

	useEffect(() => manager.setShowHidden(showHidden), [manager, showHidden]);

	useEffect(() => {
		if (!notice) return;
		const timer = setTimeout(() => setNotice(null), NOTICE_MS);
		return () => clearTimeout(timer);
	}, [notice]);

	useSyncExternalStore(manager.subscribe, manager.getVersion);
	const tab = navigation.tab;
	// A tab that has only just become active has no listing until the effect above opens one.
	const state = tab ? (manager.stateFor(tab.id) ?? OPENING) : undefined;
	const session = state?.status === 'ready' ? state.session : null;

	// A menu belongs to the entry and listing it was opened on; it must not outlive either.
	const handle = session?.model.handle;
	useEffect(() => setMenu(null), [tab?.id, handle]);

	return (
		<div className={styles.workspace}>
			<TabStrip />
			<NavigationBar leading={<SidebarToggle />} />
			<div className={styles.middle}>
				{sidebarOpen && <Sidebar showHidden={showHidden} onNotice={notify} />}
				<div
					className={styles.files}
					role="tabpanel"
					id={TAB_PANEL_ID}
					aria-label={tab ? undefined : t('tabs.panel.label')}
					aria-labelledby={tab ? tabDomId(tab.id) : undefined}
				>
					{state && (
						<FileView
							state={state}
							mode={mode}
							gridSize={gridSize}
							onOpen={open}
							onOpenInNewTab={openInNewTab}
							onMenu={setMenu}
						/>
					)}
				</div>
			</div>
			<StatusBar session={session} location={tab?.location} notice={notice?.text ?? null}>
				<ViewSwitcher />
			</StatusBar>
			{menu?.kind === 'background' && (
				<BackgroundContextMenu
					session={session}
					showHidden={showHidden}
					position={menu.position}
					keyboard={menu.keyboard}
					onToggleHidden={() => viewStore.getState().toggleHidden()}
					onClose={() => setMenu(null)}
				/>
			)}
			{menu?.kind === 'entry' && (
				<EntryContextMenu
					entry={menu.entry}
					handle={menu.handle}
					position={menu.position}
					keyboard={menu.keyboard}
					onClose={() => setMenu(null)}
					onOpen={open}
					onOpenInNewTab={openInNewTab}
					onCopyPath={copyPath}
					onAddToFavourites={addToFavourites}
				/>
			)}
		</div>
	);
}
