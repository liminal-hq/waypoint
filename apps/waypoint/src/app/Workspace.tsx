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
import type { SessionState } from '../browse/useListingSession';
import { useVfsClient } from '../browse/VfsClientContext';
import { useViewShortcuts } from '../browse/useViewShortcuts';
import { flushHints, followHints, HINT_INTERVAL_MS } from '../browse/tabHints';
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
import { useAddFavourite } from '../sidebar/useAddFavourite';
import { useSidebarShortcuts } from '../sidebar/useSidebarShortcuts';
import { NavigationBar } from '../nav/NavigationBar';
import { useNavigation } from '../nav/useNavigation';
import type { EntryAction } from '../nav/useOpenEntry';
import { StatusBar } from '../status/StatusBar';
import { ViewSwitcher } from '../status/ViewSwitcher';
import type { TearoffClient } from '../services/tearoffClient';
import { TabDragProvider, type TearOffFactory } from '../tabs/TabDragContext';
import { announce } from '../tabs/announcer';
import { createTearCardStore } from '../tabs/tearOffCardModel';
import { TearOffCard } from '../tabs/TearOffCard';
import { createTearOff } from '../tabs/tearOff';
import { sessionSignature } from '../tabs/tabDrag';
import { MergeLandingContext } from '../tabs/MergeLandingContext';
import { createMergeLandingStore } from '../tabs/mergeLanding';
import { useMergeLanding } from '../tabs/useMergeLanding';
import { useDropRegions, useTearHandoff, useTearoffFeatures } from '../tabs/useTearoff';
import { TabStrip } from '../tabs/TabStrip';
import { activePair, visibleTabs } from '../tabs/pairLayout';
import { tabDomId, TAB_PANEL_ID } from '../tabs/tabIds';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { onNotice } from '../tabs/notices';
import { usePairShortcuts } from '../tabs/usePairShortcuts';
import { useTabShortcuts } from '../tabs/useTabShortcuts';
import { useWindowShortcuts } from '../tabs/windowActions';
import { NoticeToast } from './NoticeToast';
import { clearPaneFocus } from '../tabs/paneFocus';
import { dismissNotice } from './notices';
import { PaneArea, type PaneMenuRequest } from './PaneArea';
import styles from './Workspace.module.css';

const OPENING: SessionState = { status: 'opening' };

/** How long a failure stays in the status bar. */
const NOTICE_MS = 6000;

/** What the window starts from: the saved view, and a sentence to show when the last session could not be restored. */
export interface WorkspaceStartup {
	view?: ViewPrefs;
	notice?: string | null;
}

/** The transparent margin the window frame draws around the content, in logical pixels (0 where the OS draws the frame). */
function frameMargin(): number {
	const value = getComputedStyle(document.documentElement).getPropertyValue(
		'--wp-window-shadow-margin',
	);
	const margin = Number.parseFloat(value);
	return Number.isFinite(margin) ? margin : 0;
}

/**
 * The browsing area. It owns the window's view choices (list or grid, icon size, hidden files),
 * and the tear-off hook that the tab drag's new-window phase runs on: without a `tearoff` client
 * (the in-memory demo) a release outside the strip does nothing.
 */
export function Workspace({
	startup,
	tearoff,
}: {
	startup?: WorkspaceStartup;
	tearoff?: TearoffClient;
}) {
	const [viewStore] = useState(() =>
		createViewStore(startup?.view ? viewFromPrefs(startup.view) : {}),
	);
	const [sidebarStore] = useState(() => createSidebarStore());
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const { features, live } = useTearoffFeatures(tearoff);
	// The regions are what a drop on this window merges at, for a ghost's hit test and for a window drag the compositor runs.
	useDropRegions(tearoff, features.hitTest || features.toplevelDrag, sessionSignature(snapshot));
	const [landing] = useState(createMergeLandingStore);
	useMergeLanding(tearoff, landing, announce, snapshot);
	useTearHandoff(tearoff, { api, flush: flushHints, announce });
	const [card] = useState(createTearCardStore);
	const latest = useRef(snapshot);
	latest.current = snapshot;
	// The hook is made once with the drag session; it reads what changes through these.
	const makeTearOff = useCallback<TearOffFactory>(
		(control) =>
			createTearOff({
				client: tearoff as TearoffClient,
				features: () => live.current,
				api,
				snapshot: () => latest.current,
				flush: flushHints,
				announce,
				cancelDrag: control.cancel,
				card,
				viewport: () => ({ width: window.innerWidth, height: window.innerHeight }),
				frameMargin,
			}),
		[tearoff, live, api, card],
	);
	return (
		<ViewStoreContext.Provider value={viewStore}>
			<SidebarStoreContext.Provider value={sidebarStore}>
				{/* A tab drag's state is shared by the strip and the file area, so it starts here. */}
				<TabDragProvider tearOff={tearoff ? makeTearOff : undefined}>
					<MergeLandingContext.Provider value={landing}>
						<TearOffCard store={card} />
						<WorkspaceBody
							viewStore={viewStore}
							sidebarStore={sidebarStore}
							startupNotice={startup?.notice ?? null}
						/>
					</MergeLandingContext.Provider>
				</TabDragProvider>
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
	const [menu, setMenu] = useState<PaneMenuRequest | null>(null);
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
	const addFavourite = useAddFavourite();
	// A message from the sidebar, numbered like the others so a repeat restarts the timer.
	const notify = useCallback((text: string) => setNotice({ id: ++noticeCount.current, text }), []);
	const pinCurrent = useCallback(
		(location: Location) => {
			addFavourite(location).catch((error: unknown) => {
				console.warn('could not add the favourite', error);
				notify(t('sidebar.favourites.failed'));
			});
		},
		[addFavourite, notify],
	);
	useSidebarShortcuts(sidebarStore, navigation.tab?.location, pinCurrent);
	const sidebarOpen = useSidebarState((state) => state.open);
	useTabShortcuts();
	usePairShortcuts();
	useWindowShortcuts();
	// Messages from code with no route to the status bar (a window that could not open).
	useEffect(() => onNotice(notify), [notify]);
	useViewShortcuts(viewStore);
	const mode = useViewState((view) => view.mode);
	const gridSize = useViewState((view) => view.gridSize);
	const showHidden = useViewState((view) => view.showHidden);

	// One listing per tab, kept in step with the session (A9, A20): every pane on screen has a live one.
	useEffect(() => {
		if (snapshot) manager.sync(snapshot.tabs, visibleTabs(snapshot));
	}, [manager, snapshot]);
	useEffect(
		() => () => {
			manager.dispose();
			// A toast or a focus request from this window's session means nothing to the next one.
			dismissNotice();
			clearPaneFocus();
		},
		[manager],
	);
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
	// The panes on screen: the active tab, or all of its pair. The focused pane is the active tab.
	const pair = activePair(snapshot);
	const panes = pair
		? pair.panes.flatMap((id) => snapshot?.tabs.find((candidate) => candidate.id === id) ?? [])
		: tab
			? [tab]
			: [];
	// A tab that has only just become visible has no listing until the effect above opens one.
	const stateFor = (id: number): SessionState => manager.stateFor(id) ?? OPENING;
	const state = tab ? stateFor(tab.id) : undefined;
	const session = state?.status === 'ready' ? state.session : null;

	// A menu belongs to the entry and listing it was opened on; it must not outlive either. Another
	// pane becoming active (the press that opened the menu does that) is not a reason to close it.
	const liveHandles = panes
		.map((pane) => {
			const paneState = manager.stateFor(pane.id);
			return paneState?.status === 'ready' ? paneState.session.model.handle : '';
		})
		.join(',');
	useEffect(() => {
		const handle = menu?.session?.model.handle;
		if (menu && (handle === undefined || !liveHandles.split(',').includes(String(handle)))) {
			setMenu(null);
		}
	}, [menu, liveHandles]);

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
					{panes.length > 0 && (
						<PaneArea
							panes={panes}
							pair={panes.length > 1 ? pair : undefined}
							active={tab?.id ?? null}
							stateFor={stateFor}
							mode={mode}
							gridSize={gridSize}
							onFailure={onFailure}
							onMenu={setMenu}
						/>
					)}
				</div>
			</div>
			<StatusBar session={session} location={tab?.location} notice={notice?.text ?? null}>
				<ViewSwitcher />
			</StatusBar>
			<NoticeToast />
			{menu?.kind === 'background' && (
				<BackgroundContextMenu
					session={menu.session}
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
					onOpen={menu.openers.open}
					onOpenInNewTab={menu.openers.openInNewTab}
					onCopyPath={menu.openers.copyPath}
					onAddToFavourites={menu.openers.addToFavourites}
				/>
			)}
		</div>
	);
}
