// The browsing area of the Main window: tabs and toolbar over the active tab's file view and status bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { useStore } from 'zustand';
import { EntryContextMenu, type EntryCommand } from '../browse/EntryContextMenu';
import { TrashEntryMenu } from '../trash/TrashEntryMenu';
import { useTrashClient } from '../trash/TrashClientContext';
import { TrashActionsProvider, useTrashJobs } from '../trash/trashJobs';
import { selectedCount } from '../browse/selection';
import { FolderViewController, folderViewKey } from '../browse/folderViewController';
import { IDLE_FOLDER_VIEWS, NO_FOLDERS } from '../browse/folderViewStore';
import { useFolderViews } from '../browse/FolderViewsContext';
import { ListingManager } from '../browse/listingManager';
import { useSettings, useSettingsReady } from '../settings/SettingsContext';
import { BackgroundContextMenu, type BackgroundCommand } from '../browse/BackgroundContextMenu';
import type { ListingSession, SessionState } from '../browse/useListingSession';
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
import type { NativeDndClient } from '../services/nativeDndClient';
import type { ShelfWindowClient } from '../services/shelfWindowClient';
import type { TearoffClient } from '../services/tearoffClient';
import { TabDragProvider, type TearOffFactory } from '../tabs/TabDragContext';
import { announce } from '../tabs/announcer';
import { ConnectHost } from '../connections/ConnectHost';
import { useConnections } from '../connections/ConnectionsContext';
import type { ConnectionsView } from '../connections/connectionsModel';
import { RetryProvider } from '../connections/RemoteState';
import { isConnectionError } from '../connections/remoteModel';
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
import { ActiveTabWindowTitle } from './ActiveTabWindowTitle';
import { onNotice } from '../tabs/notices';
import { usePairShortcuts } from '../tabs/usePairShortcuts';
import { useTabShortcuts } from '../tabs/useTabShortcuts';
import { useWindowShortcuts } from '../tabs/windowActions';
import { CloseGuardHost } from '../tabs/CloseGuardHost';
import { FileDragProvider } from '../dnd/FileDragContext';
import { OpenWithHost } from '../openWith/OpenWithHost';
import { QuickLookHost } from '../quicklook/QuickLookHost';
import { InspectorProvider } from '../inspector/InspectorContext';
import { usePropertiesWindowClient } from '../inspector/PropertiesWindowContext';
import { usePropertiesWindowHost } from '../inspector/usePropertiesWindowHost';
import { InspectorDock } from '../inspector/InspectorPanel';
import { createInspectorStore } from '../inspector/inspectorStore';
import { ShelfProvider } from '../shelf/ShelfContext';
import { ShelfDock, ShelfToggle } from '../shelf/ShelfToggle';
import { useFileCommandsHost } from '../ops/useFileCommandsHost';
import { FileCommandsProvider } from '../ops/FileCommandsContext';
import { useFileShortcuts } from '../ops/useFileShortcuts';
import { ClipboardProvider } from '../ops/ClipboardContext';
import { CompressHost } from '../ops/CompressHost';
import { DestinationHost } from '../ops/DestinationHost';
import { otherPaneSession } from '../ops/otherPane';
import { createTauriBatchRenameApi } from '../ops/batchRename/tauriBatchRenameApi';
import { BatchRenameHost } from '../ops/batchRename/BatchRenameHost';
import { batchRenameSelection } from '../ops/batchRename/batchRenameSelection';
import { openBatchRename } from '../ops/batchRename/batchRenameStore';
import { useBatchRenameShortcut } from '../ops/batchRename/useBatchRenameShortcut';
import {
	CommandBridgeProvider,
	createCommandBridge,
	useCommandBridge,
	useProvidedCommandBridge,
} from '../commands/commandBridge';
import { useWorkspaceCommands } from '../commands/useWorkspaceCommands';
import { ActionBar } from './ActionBar';
import { frameMargin } from './frameMargin';
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

/**
 * The browsing area. It owns the window's view choices (list or grid, icon size, hidden files),
 * and the tear-off hook that the tab drag's new-window phase runs on: without a `tearoff` client
 * (the in-memory demo) a release outside the strip does nothing.
 */
export function Workspace({
	startup,
	tearoff,
	nativeDnd,
	shelfWindow,
}: {
	startup?: WorkspaceStartup;
	tearoff?: TearoffClient;
	/** The native drag and drop plugin: files dragged in from other applications, and drags that leave the window. */
	nativeDnd?: NativeDndClient;
	/** Raises and hides the Shelf window while the Shelf is undocked; the real one when omitted. */
	shelfWindow?: ShelfWindowClient;
}) {
	const [viewStore] = useState(() =>
		createViewStore(startup?.view ? viewFromPrefs(startup.view) : {}),
	);
	const [sidebarStore] = useState(() => createSidebarStore());
	// The Main window provides the bridge its title bar's menu reads; a workspace mounted alone makes its own.
	const [ownBridge] = useState(createCommandBridge);
	const bridge = useProvidedCommandBridge() ?? ownBridge;
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
		<CommandBridgeProvider value={bridge}>
			<ViewStoreContext.Provider value={viewStore}>
				<SidebarStoreContext.Provider value={sidebarStore}>
					{/* A tab drag's state is shared by the strip and the file area, so it starts here. */}
					<TabDragProvider tearOff={tearoff ? makeTearOff : undefined}>
						<MergeLandingContext.Provider value={landing}>
							<ActiveTabWindowTitle />
							<TearOffCard store={card} />
							<CloseGuardHost>
								<WorkspaceBody
									viewStore={viewStore}
									sidebarStore={sidebarStore}
									startupNotice={startup?.notice ?? null}
									nativeDnd={nativeDnd}
									shelfWindow={shelfWindow}
								/>
							</CloseGuardHost>
						</MergeLandingContext.Provider>
					</TabDragProvider>
				</SidebarStoreContext.Provider>
			</ViewStoreContext.Provider>
		</CommandBridgeProvider>
	);
}

function WorkspaceBody({
	viewStore,
	sidebarStore,
	startupNotice,
	nativeDnd,
	shelfWindow,
}: {
	viewStore: ViewStore;
	sidebarStore: SidebarStore;
	startupNotice: string | null;
	nativeDnd: NativeDndClient | undefined;
	shelfWindow: ShelfWindowClient | undefined;
}) {
	const client = useVfsClient();
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	// The Inspector's memory is the window's; the right-click menus open it, so the workspace holds it.
	const [inspectorStore] = useState(createInspectorStore);
	// What each folder shows: its own remembered view and sort, or the window's (SPEC 5.3b).
	const [folderViews] = useState(() => new FolderViewController(viewStore));
	const [manager] = useState(
		() =>
			new ListingManager(client, {
				// A new listing opens with its folder's remembered sort and grouping, or else the
				// window's, and the hidden-files choice. A folder listing's change of sort is
				// remembered by the folder, or becomes the window's when remembering is off (saved
				// with its view).
				viewMode: () => viewStore.getState().mode,
				openOptions: (_inherited, location) => ({
					sort: folderViews.sortFor(location),
					filter: { showHidden: viewStore.getState().showHidden },
				}),
				onSort: (sort, location) => folderViews.onSort(sort, location),
			}),
	);
	// Numbered, so the same message arriving again restarts its timer.
	const [notice, setNotice] = useState<{ id: number; text: string } | null>(
		startupNotice ? { id: 0, text: startupNotice } : null,
	);
	const noticeCount = useRef(0);
	const [menu, setMenu] = useState<PaneMenuRequest | null>(null);
	const navigation = useNavigation();
	const folderViewsHandle = useFolderViews();
	const rememberSetting = useSettings((settings) => settings.general.rememberFolderViews);
	const settingsReady = useSettingsReady();
	const remembering = rememberSetting && settingsReady && folderViewsHandle !== null;
	const activeKey = folderViewKey(navigation.tab?.location);
	folderViews.configure({ handle: folderViewsHandle, enabled: remembering, activeKey });
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
	// The Trash view's jobs and the questions they ask; the sidebar, the menus and the Trash's own
	// strip all reach them through `TrashActionsProvider`.
	const { actions: trashActions, dialogs: trashDialogs } = useTrashJobs(useTrashClient(), notify);
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
	// The folders' remembered views: opening a folder shows its own, and a choice made in one is
	// kept. What the folders remember, and whether this window's own writes are all answered, are
	// what bring the open listings and the view mode back in line.
	const bridge = useCommandBridge();
	const folderViewStore = folderViewsHandle?.store ?? IDLE_FOLDER_VIEWS;
	const remembered = useStore(folderViewStore, (state) =>
		remembering ? state.folders : NO_FOLDERS,
	);
	const written = useStore(folderViewStore, (state) => state.writing === 0);
	useEffect(() => folderViews.followMode(), [folderViews]);
	useEffect(() => {
		folderViews.reconcile(manager);
	}, [folderViews, manager, remembering, activeKey, remembered, written]);
	const folderViewState =
		activeKey === null || !remembering
			? 'unavailable'
			: remembered.has(activeKey)
				? 'remembered'
				: 'default';
	useEffect(() => {
		bridge.patchFacts({ folderView: folderViewState });
	}, [bridge, folderViewState]);
	useEffect(() => {
		bridge.patchActions({ resetFolderView: () => void folderViews.resetActive() });
	}, [bridge, folderViews]);
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
	// The file commands act on the active pane's listing; the keys read it when pressed.
	const activeSession = useCallback(() => {
		const id = activeId.current;
		const state = id === null ? undefined : manager.stateFor(id);
		return state?.status === 'ready' ? state.session : null;
	}, [manager]);
	// F5 and Shift+F5 copy and move to the pane beside the one that has the selection.
	const snapshotRef = useRef(snapshot);
	snapshotRef.current = snapshot;
	const otherPane = useCallback(
		(from: ListingSession) =>
			otherPaneSession(snapshotRef.current, from, (id) => manager.stateFor(id)),
		[manager],
	);
	const {
		commands,
		dialog: commandDialog,
		clipboard,
	} = useFileCommandsHost(activeSession, otherPane);
	useFileShortcuts(commands, {
		activeSession,
		deleteInTrash: (session) => trashActions?.deletePermanently(session),
		restoreInTrash: (session) => trashActions?.restore(session),
	});
	// The menu, the Action bar and (next) the palette list the same commands; the bridge carries their state.
	useWorkspaceCommands({
		commands,
		activeSession,
		subscribePanes: manager.subscribe,
		clipboard,
		view: viewStore,
		sidebar: sidebarStore,
		trash: trashActions,
	});
	// Alt+Enter, the item menu and the palette open a Properties window for the active pane's subject.
	const openProperties = usePropertiesWindowHost(bridge, activeSession);
	const propertiesWindowAvailable = usePropertiesWindowClient() !== null;
	// Ctrl+F2 batch renames the active pane's selection, where the listing can be written to.
	const batchRenameApi = useMemo(createTauriBatchRenameApi, []);
	const currentBatchSelection = useCallback(
		() => batchRenameSelection(activeSession()),
		[activeSession],
	);
	useBatchRenameShortcut(currentBatchSelection);
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

	// The folder a background menu belongs to (the pane it was opened in, which may not be the active one).
	const menuFolderKey = folderViewKey(menu?.session?.model.location);
	const runCommand = (command: EntryCommand | BackgroundCommand, entry?: Entry) => {
		const from = menu?.session ?? null;
		if (command === 'properties') return inspectorStore.getState().showProperties();
		if (command === 'propertiesWindow') return openProperties(from, entry);
		if (!commands) return;
		switch (command) {
			case 'newFolder':
				return void commands.newFolder(from);
			case 'newFile':
				return void commands.newFile(from);
			case 'rename':
				return commands.rename(from, entry);
			case 'batchRename': {
				const selection = batchRenameSelection(from);
				return selection ? openBatchRename(selection) : undefined;
			}
			case 'duplicate':
				return void commands.duplicate(from);
			case 'extractHere':
				return void commands.extractHere(from);
			case 'extractTo':
				return void commands.extractTo(from);
			case 'compress':
				return void commands.compress(from);
			case 'cut':
				return void commands.cut(from);
			case 'copy':
				return void commands.copy(from);
			case 'paste':
				return void commands.paste(from);
			case 'pasteInto':
				return void commands.paste(from, entry);
			case 'copyTo':
				return void commands.copyTo(from);
			case 'moveTo':
				return void commands.moveTo(from);
			case 'copyToOtherPane':
				return void commands.copyToOtherPane(from);
			case 'moveToOtherPane':
				return void commands.moveToOtherPane(from);
			case 'moveToTrash':
				return void commands.moveToTrash(from);
			case 'deletePermanently':
				return void commands.deletePermanently(from);
			case 'undo':
				return void commands.undo();
			case 'redo':
				return void commands.redo();
		}
	};

	// A folder whose server could not be reached opens again once its login is back, wherever it
	// was reconnected from (the folder's Reconnect, the Network section, the Connect dialog).
	const retryRemote = useCallback(() => {
		manager.retryFailed(isConnectionError);
	}, [manager]);
	const connections = useConnections();
	useEffect(() => {
		if (!connections) return;
		let before = connections.store.getState().states;
		return connections.store.subscribe((view) => {
			const now = view.states;
			if (now === before) return;
			let back = false;
			for (const [key, state] of now) {
				if (state.kind === 'connected' && before.get(key)?.kind !== 'connected') back = true;
			}
			before = now;
			if (back) retryRemote();
		});
	}, [connections, retryRemote]);

	// A protocol turned off in Settings → Experimental ends the listings of its tabs at once.
	useEffect(() => {
		if (!connections) return;
		const apply = (view: ConnectionsView) => {
			if (view.protocols) manager.applyProtocols(view.protocols.off);
		};
		apply(connections.store.getState());
		let before = connections.store.getState().protocols;
		return connections.store.subscribe((view) => {
			if (view.protocols === before) return;
			before = view.protocols;
			apply(view);
		});
	}, [connections, manager]);

	return (
		<RetryProvider retry={retryRemote}>
			<TrashActionsProvider value={trashActions}>
				<FileCommandsProvider value={commands}>
					<ClipboardProvider value={clipboard}>
						<InspectorProvider store={inspectorStore}>
							<ShelfProvider
								activeSession={activeSession}
								{...(shelfWindow ? { windowClient: shelfWindow } : {})}
							>
								<FileDragProvider manager={manager} nativeDnd={nativeDnd}>
									<div className={styles.workspace}>
										<TabStrip />
										<NavigationBar />
										<ActionBar />
										<div className={styles.middle}>
											{sidebarOpen && <Sidebar showHidden={showHidden} onNotice={notify} />}
											<div className={styles.content}>
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
												<ShelfDock />
											</div>
											<InspectorDock session={session} location={tab?.location} />
										</div>
										<StatusBar
											session={session}
											location={tab?.location}
											notice={notice?.text ?? null}
										>
											<ViewSwitcher />
											<ShelfToggle />
										</StatusBar>
										<NoticeToast />
										{commandDialog}
										<BatchRenameHost api={batchRenameApi} announce={notify} />
										<ConnectHost />
										<DestinationHost />
										<CompressHost />
										<OpenWithHost />
										<QuickLookHost />
										{trashDialogs}
										{menu?.kind === 'background' && (
											<BackgroundContextMenu
												session={menu.session}
												showHidden={showHidden}
												position={menu.position}
												keyboard={menu.keyboard}
												onToggleHidden={() => viewStore.getState().toggleHidden()}
												onEmptyTrash={
													trashActions
														? () => trashActions.emptyTrash(menu.session?.model.count ?? 0)
														: undefined
												}
												onClose={() => setMenu(null)}
												commands={
													commands
														? {
																states: commands.states(menu.session),
																undoLabel: commands.history().undo?.label ?? null,
																redoLabel: commands.history().redo?.label ?? null,
															}
														: undefined
												}
												folderView={
													remembering && menuFolderKey !== null
														? remembered.has(menuFolderKey)
															? 'remembered'
															: 'default'
														: undefined
												}
												onResetFolderView={() => void folderViews.reset(menuFolderKey)}
												onCommand={runCommand}
											/>
										)}
										{menu?.kind === 'entry' &&
											menu.session?.model.layout === 'trash' &&
											trashActions && (
												<TrashEntryMenu
													position={menu.position}
													keyboard={menu.keyboard}
													onRestore={() => menu.session && trashActions.restore(menu.session)}
													onDelete={() =>
														menu.session && trashActions.deletePermanently(menu.session)
													}
													onClose={() => setMenu(null)}
												/>
											)}
										{menu?.kind === 'entry' && menu.session?.model.layout !== 'trash' && (
											<EntryContextMenu
												entry={menu.entry}
												handle={menu.handle}
												position={menu.position}
												keyboard={menu.keyboard}
												onClose={() => setMenu(null)}
												onOpen={menu.openers.open}
												onOpenInNewTab={menu.openers.openInNewTab}
												onCopyPath={menu.openers.copyPath}
												session={menu.session}
												onAddToFavourites={menu.openers.addToFavourites}
												commands={commands?.states(menu.session)}
												batchRename={
													menu.session
														? selectedCount(
																menu.session.store.getState().selection,
																menu.session.model.count,
															) > 1
														: false
												}
												propertiesWindow={propertiesWindowAvailable}
												onCommand={runCommand}
											/>
										)}
									</div>
								</FileDragProvider>
							</ShelfProvider>
						</InspectorProvider>
					</ClipboardProvider>
				</FileCommandsProvider>
			</TrashActionsProvider>
		</RetryProvider>
	);
}
