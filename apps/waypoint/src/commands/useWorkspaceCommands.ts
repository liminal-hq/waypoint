// Connects a Main window's workspace to the command bridge: the facts that follow its panes and the actions over its hooks
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef } from 'react';
import { showNotice } from '../app/notices';
import type { ListingSession } from '../browse/useListingSession';
import type { ViewStore } from '../browse/viewStore';
import { tf } from '../i18n/messages';
import { batchRenameSelection } from '../ops/batchRename/batchRenameSelection';
import { openBatchRename } from '../ops/batchRename/batchRenameStore';
import type { ClipboardService } from '../ops/clipboardService';
import type { FileCommands } from '../ops/fileCommands';
import { useOps } from '../ops/OpsContext';
import { isSettingsError } from '../services/settingsClient';
import { useSettings, useSettingsHandle } from '../settings/SettingsContext';
import { usePlacesClient } from '../sidebar/PlacesClientContext';
import type { SidebarStore } from '../sidebar/sidebarStore';
import { usePlaces } from '../sidebar/usePlaces';
import { usePairActions } from '../tabs/pairActions';
import { pairOfTab } from '../tabs/pairLayout';
import { useTabActions } from '../tabs/tabActions';
import { useTabExtras } from '../tabs/tabExtras';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import type { TrashActions } from '../trash/trashJobs';
import { useWindowActions } from '../tabs/windowActions';
import { useCommandBridge } from './commandBridge';
import { startCommandFeed, type CommandFeed } from './commandFeed';

export interface WorkspaceCommandSources {
	commands: FileCommands | null;
	/** The pane the commands act on. */
	activeSession: () => ListingSession | null;
	/** Hears a pane's listing opening or closing. */
	subscribePanes: (listener: () => void) => () => void;
	clipboard: ClipboardService | null;
	view: ViewStore;
	sidebar: SidebarStore;
	/** The Trash view's jobs, `null` where the window has no Trash service. */
	trash?: TrashActions | null;
}

/**
 * Called once by the workspace. It starts the feed for the facts the panes, queue, clipboard and
 * view decide, patches in the facts the tabs and settings decide, and gives the bridge the actions:
 * each is the call the key or menu already makes (`useTabActions`, `usePairActions`, the view and
 * sidebar stores), read through a ref so the bridge's actions never go stale and are not rebuilt
 * on every selection change.
 */
export function useWorkspaceCommands(sources: WorkspaceCommandSources): void {
	const bridge = useCommandBridge();
	const ops = useOps();
	const snapshot = useTabsSnapshot();
	const tabActions = useTabActions();
	const extras = useTabExtras();
	const pairActions = usePairActions();
	const windows = useWindowActions();
	const settings = useSettingsHandle();
	const ui = useSettings((value) => value.ui);
	const tabsApi = useTabsApi();
	const places = usePlaces(usePlacesClient());
	const feed = useRef<CommandFeed | null>(null);

	const latest = useRef({
		...sources,
		snapshot,
		tabActions,
		extras,
		pairActions,
		windows,
		settings,
		tabsApi,
		places,
	});
	latest.current = {
		...sources,
		snapshot,
		tabActions,
		extras,
		pairActions,
		windows,
		settings,
		tabsApi,
		places,
	};

	const { commands, clipboard, view, sidebar, subscribePanes } = sources;
	const handle = ops?.handle ?? null;
	useEffect(() => {
		// `activeSession` is read through the ref: it changes identity with the render, not with the pane.
		const started = startCommandFeed(bridge, {
			files: commands,
			activeSession: () => latest.current.activeSession(),
			ops: handle,
			clipboard,
			view,
			sidebar,
			subscribePanes,
		});
		feed.current = started;
		return () => {
			started.stop();
			feed.current = null;
		};
	}, [bridge, commands, handle, clipboard, view, sidebar, subscribePanes]);

	const active = snapshot?.active ?? null;
	const paired = pairOfTab(snapshot?.pairs ?? [], active) !== undefined;
	const tabCount = snapshot?.tabs.length ?? 0;
	useEffect(() => {
		bridge.patchFacts({
			tab: active !== null,
			tabCount,
			paired,
			actionBar: ui.actionBar,
			actionBarLabels: ui.actionBarLabels,
		});
		// Another tab's pane is now the one the file commands act on.
		feed.current?.refresh();
	}, [bridge, active, tabCount, paired, ui.actionBar, ui.actionBarLabels]);

	// The places the "Go to" commands open; a place the sidebar does not have is not offered.
	const placeKinds = (places?.places ?? []).map((place) => place.kind);
	const placeKey = placeKinds.join(',');
	useEffect(() => {
		bridge.patchFacts({
			places: placeKey === '' ? [] : (placeKey.split(',') as typeof placeKinds),
			// Making a link on Windows needs a privilege the person may not hold, and only fails per item.
			linkSupported: document.documentElement.dataset.platform !== 'windows',
		});
	}, [bridge, placeKey]);

	useEffect(() => {
		const activeId = () => latest.current.snapshot?.active ?? null;
		const activeTab = () => latest.current.snapshot?.tabs.find((tab) => tab.id === activeId());
		const saveUi = (change: Partial<{ actionBar: boolean; actionBarLabels: boolean }>) => {
			const handle = latest.current.settings;
			if (!handle) return;
			// Only the Action bar's own fields go to Rust, which merges them into what is in force: this
			// window's copy of the rest may be older than the Settings window's.
			handle.saveUi(change).catch((error: unknown) => {
				console.warn('could not save the Action bar choice', error);
				showNotice(
					tf('settings.error.saveFailed', {
						reason: isSettingsError(error) ? error.message : String(error),
					}),
				);
			});
		};
		bridge.patchActions({
			files: commands,
			restoreFromTrash: () => {
				const session = latest.current.activeSession();
				if (session) latest.current.trash?.restore(session);
			},
			deleteFromTrash: () => {
				const session = latest.current.activeSession();
				if (session) latest.current.trash?.deletePermanently(session);
			},
			batchRename: () => {
				const selection = batchRenameSelection(latest.current.activeSession());
				if (selection) openBatchRename(selection);
			},
			selectAll: () => latest.current.activeSession()?.store.getState().selectAll(),
			invertSelection: () => latest.current.activeSession()?.store.getState().invertSelection(),
			changeSort: (change) => {
				const session = latest.current.activeSession();
				if (session) void session.model.setSort(change(session.model.sort));
			},
			undoEntry: (entry) => void latest.current.commands?.undoEntry(entry),
			redoEntry: (entry) => void latest.current.commands?.redoEntry(entry),
			newWindow: () => void latest.current.windows.newWindow(),
			newTab: () => latest.current.tabActions.newTab(),
			closeTab: () => {
				const id = activeId();
				if (id !== null) latest.current.tabActions.close(id);
			},
			reopenClosedTab: () => latest.current.extras.reopen(),
			duplicateTab: () => {
				const tab = activeTab();
				if (tab) latest.current.extras.duplicate(tab);
			},
			toggleSplit: () => {
				const id = activeId();
				if (id !== null) latest.current.pairActions.toggleSplit(id);
			},
			moveTabToNewWindow: () => {
				const tab = activeTab();
				if (tab) void latest.current.windows.moveToNewWindow(tab);
			},
			goToPlace: (kind) => {
				const place = latest.current.places?.places.find((candidate) => candidate.kind === kind);
				const id = activeId();
				if (place && id !== null) {
					latest.current.tabsApi.navigate(id, place.location).catch((error: unknown) => {
						console.warn('could not open the place', error);
					});
				}
			},
			setViewMode: (mode) => latest.current.view.getState().setMode(mode),
			toggleHidden: () => latest.current.view.getState().toggleHidden(),
			toggleSidebar: () => latest.current.sidebar.getState().toggleOpen(),
			setActionBar: (shown) => saveUi({ actionBar: shown }),
			setActionBarLabels: (shown) => saveUi({ actionBarLabels: shown }),
		});
	}, [bridge, commands]);
}
