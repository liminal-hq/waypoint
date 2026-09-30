// The browsing area of the Main window: tabs and toolbar over the active tab's file view and status bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from 'react';
import { EntryContextMenu } from '../browse/EntryContextMenu';
import { ListingManager } from '../browse/listingManager';
import { BackgroundContextMenu } from '../browse/BackgroundContextMenu';
import { FileView } from '../browse/FileView';
import type { MenuRequest } from '../browse/useListInteractions';
import type { SessionState } from '../browse/useListingSession';
import { useVfsClient } from '../browse/VfsClientContext';
import { useViewShortcuts } from '../browse/useViewShortcuts';
import {
	createViewStore,
	useViewState,
	ViewStoreContext,
	type ViewStore,
} from '../browse/viewStore';
import { tf, t } from '../i18n/messages';
import { NavigationBar } from '../nav/NavigationBar';
import { useNavigation } from '../nav/useNavigation';
import { useOpenEntry, type EntryAction } from '../nav/useOpenEntry';
import { StatusBar } from '../status/StatusBar';
import { ViewSwitcher } from '../status/ViewSwitcher';
import { TabStrip } from '../tabs/TabStrip';
import { tabDomId, TAB_PANEL_ID } from '../tabs/tabIds';
import { useTabsSnapshot } from '../tabs/TabsContext';
import { useTabShortcuts } from '../tabs/useTabShortcuts';
import styles from './Workspace.module.css';

const OPENING: SessionState = { status: 'opening' };

/** How long a failure stays in the status bar. */
const NOTICE_MS = 6000;

/** The browsing area. It owns the window's view choices (list or grid, icon size, hidden files). */
export function Workspace() {
	const [viewStore] = useState(() => createViewStore());
	return (
		<ViewStoreContext.Provider value={viewStore}>
			<WorkspaceBody viewStore={viewStore} />
		</ViewStoreContext.Provider>
	);
}

function WorkspaceBody({ viewStore }: { viewStore: ViewStore }) {
	const client = useVfsClient();
	const snapshot = useTabsSnapshot();
	const [manager] = useState(
		() =>
			new ListingManager(client, {
				// A new listing keeps the hidden-files choice; the sort is inherited from the
				// listing the tab had before.
				openOptions: (inherited) => ({
					...(inherited ? { sort: inherited } : {}),
					filter: { showHidden: viewStore.getState().showHidden },
				}),
			}),
	);
	// Numbered, so the same message arriving again restarts its timer.
	const [notice, setNotice] = useState<{ id: number; text: string } | null>(null);
	const noticeCount = useRef(0);
	const [menu, setMenu] = useState<MenuRequest | null>(null);
	const navigation = useNavigation();
	const onFailure = useCallback(
		(entry: Entry, action: EntryAction) =>
			setNotice({
				id: ++noticeCount.current,
				text: tf(action === 'copyPath' ? 'status.copyPathFailed' : 'status.openFailed', {
					name: entry.name,
				}),
			}),
		[],
	);
	const { open, openInNewTab, copyPath } = useOpenEntry(navigation, onFailure);
	useTabShortcuts();
	useViewShortcuts(viewStore);
	const mode = useViewState((view) => view.mode);
	const gridSize = useViewState((view) => view.gridSize);
	const showHidden = useViewState((view) => view.showHidden);

	// One listing per tab, kept in step with the session (A9, A20).
	useEffect(() => {
		if (snapshot) manager.sync(snapshot.tabs, snapshot.active);
	}, [manager, snapshot]);
	useEffect(() => () => manager.dispose(), [manager]);

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
			<NavigationBar />
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
				/>
			)}
		</div>
	);
}
