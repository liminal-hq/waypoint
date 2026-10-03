// What the commands read (facts about the window now) and what they do (actions over the existing hooks)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { JournalEntrySummary } from '@liminal-hq/waypoint-protocol/generated/JournalEntrySummary';
import type { PlaceKind } from '@liminal-hq/waypoint-protocol/generated/PlaceKind';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import type { ViewMode } from '../browse/viewStore';
import type { HelpPage } from '../help/helpPages';
import {
	commandStates,
	type CommandState,
	type FileCommandId,
	type FileCommands,
} from '../ops/fileCommands';

/**
 * Everything a command's availability depends on, as plain data. The Main window's workspace keeps
 * it current (`startCommandFeed`) and the registry's `when` functions are pure over it, so a table
 * of states can be tested without a window.
 */
export interface CommandFacts {
	/** What `commandStates` says for the active pane: visibility and enablement of each file command. */
	file: Record<FileCommandId, CommandState>;
	/** How many entries of the active pane are selected. */
	selected: number;
	/** The active pane's listing is open. */
	listing: boolean;
	/** The active pane lists the Trash. */
	trash: boolean;
	/** The active pane lists a folder on this computer, which is what a link can point at. */
	local: boolean;
	/** The platform can make links without a privilege the person may not hold (not Windows). */
	linkSupported: boolean;
	/** The mime-apps plugin can list applications or has a chooser of its own, so Open With… can work here. */
	openWith: boolean;
	/** The places the sidebar offers, which "Go to" commands open. */
	places: readonly PlaceKind[];
	/** The pane can be written to and has a selection: Batch Rename can open. */
	batchRename: boolean;
	/** The active pane's sort, or `null` with no listing open. */
	sort: SortSpec | null;
	/** What Undo and Redo would do, in the history's words ("Move 3 items to Trash"). */
	undoLabel: string | null;
	redoLabel: string | null;
	/** The undo history, newest first. */
	history: readonly JournalEntrySummary[];
	/** The entry Undo would undo and the entry Redo would redo: the only two the history menu enables. */
	undoHead: number | null;
	redoHead: number | null;
	/** The window has an active tab. */
	tab: boolean;
	tabCount: number;
	/** The active tab is half of a split pair. */
	paired: boolean;
	viewMode: ViewMode;
	showHidden: boolean;
	sidebarOpen: boolean;
	actionBar: boolean;
	actionBarLabels: boolean;
	/** Whether the window manager can keep the window on top here, and whether it is. */
	alwaysOnTop: { supported: boolean; on: boolean };
	/** The Shelf panel is open, and how many items the Shelf holds. */
	shelfOpen: boolean;
	shelfCount: number;
	/** The Shelf is in its own window rather than docked in the main windows. */
	shelfUndocked: boolean;
	/** The Inspector panel is open. */
	inspectorOpen: boolean;
	/** Properties windows can be opened from here (the service exists). */
	propertiesWindow: boolean;
	/**
	 * What the folder the active tab shows remembers about its view, sort and grouping:
	 * `unavailable` where remembering is off or the folder cannot remember (the Trash, Overview),
	 * `default` when it shows the window's, `remembered` when it has its own.
	 */
	folderView: 'unavailable' | 'default' | 'remembered';
}

/** The window's actions. Each one is what a shortcut, a menu or the Action bar already does; none is a second implementation. */
export interface CommandActions {
	/** The file commands act on the active pane; `null` in a window with no queue. */
	files: FileCommands | null;
	batchRename(): void;
	/** Open With… for the active pane's selection. */
	openWith(): void;
	/** The Trash view's Restore and Delete Permanently, on the selection (the latter asks first). */
	restoreFromTrash(): void;
	deleteFromTrash(): void;
	selectAll(): void;
	invertSelection(): void;
	/** Re-sorts the active pane's listing from its current sort. */
	changeSort(change: (sort: SortSpec) => SortSpec): void;
	/** Undoes or redoes one entry of the history (what the history submenu offers). */
	undoEntry(id: number): void;
	redoEntry(id: number): void;
	newWindow(): void;
	newTab(): void;
	closeTab(): void;
	reopenClosedTab(): void;
	duplicateTab(): void;
	toggleSplit(): void;
	moveTabToNewWindow(): void;
	setViewMode(mode: ViewMode): void;
	toggleHidden(): void;
	toggleSidebar(): void;
	setActionBar(shown: boolean): void;
	setActionBarLabels(shown: boolean): void;
	setAlwaysOnTop(on: boolean): void;
	closeWindow(): void;
	openSettings(): void;
	/** Opens the command palette, with `query` typed into it. */
	openPalette(query?: string): void;
	/** Opens the Help dialog, or one of its siblings (the shortcut list, the tour, About). */
	openHelp(page: HelpPage): void;
	/** Opens one of the sidebar's places in the active tab. */
	goToPlace(place: PlaceKind): void;
	/** The Shelf panel: show or hide it, move the keyboard into it, and put the active pane's selection on it. */
	toggleShelf(): void;
	focusShelf(): void;
	addToShelf(): void;
	/** Moves the Shelf into its own window, and back into the main windows' docks. */
	undockShelf(): void;
	dockShelf(): void;
	/** The Inspector: show or hide it, and open it on the Properties tab. */
	toggleInspector(): void;
	showProperties(): void;
	/** Opens a Properties window for the active pane's one selected entry, or its folder. */
	openPropertiesWindow(): void;
	/** Makes the active folder forget its own view, sort and grouping, so it shows the window's. */
	resetFolderView(): void;
}

export interface CommandEnv {
	facts: CommandFacts;
	actions: CommandActions;
}

/** The facts of a window that has nothing open yet: the Action bar on, nothing to act on. */
export function emptyFacts(): CommandFacts {
	return {
		file: commandStates({
			queue: false,
			listing: false,
			readOnly: false,
			selected: 0,
			focused: false,
			undo: null,
			redo: null,
		}),
		selected: 0,
		listing: false,
		trash: false,
		local: false,
		linkSupported: true,
		openWith: false,
		places: [],
		batchRename: false,
		sort: null,
		undoLabel: null,
		redoLabel: null,
		history: [],
		undoHead: null,
		redoHead: null,
		tab: false,
		tabCount: 0,
		paired: false,
		viewMode: 'list',
		showHidden: false,
		sidebarOpen: true,
		actionBar: true,
		actionBarLabels: true,
		alwaysOnTop: { supported: false, on: false },
		shelfOpen: false,
		shelfCount: 0,
		shelfUndocked: false,
		inspectorOpen: false,
		propertiesWindow: false,
		folderView: 'unavailable',
	};
}

const nothing = () => {};

/** Actions that do nothing, for a window that has not mounted its workspace and for tests to override. */
export function idleActions(): CommandActions {
	return {
		files: null,
		batchRename: nothing,
		openWith: nothing,
		restoreFromTrash: nothing,
		deleteFromTrash: nothing,
		selectAll: nothing,
		invertSelection: nothing,
		changeSort: nothing,
		undoEntry: nothing,
		redoEntry: nothing,
		newWindow: nothing,
		newTab: nothing,
		closeTab: nothing,
		reopenClosedTab: nothing,
		duplicateTab: nothing,
		toggleSplit: nothing,
		moveTabToNewWindow: nothing,
		setViewMode: nothing,
		toggleHidden: nothing,
		toggleSidebar: nothing,
		setActionBar: nothing,
		setActionBarLabels: nothing,
		setAlwaysOnTop: nothing,
		closeWindow: nothing,
		openSettings: nothing,
		openPalette: nothing,
		openHelp: nothing,
		goToPlace: nothing,
		toggleShelf: nothing,
		focusShelf: nothing,
		addToShelf: nothing,
		undockShelf: nothing,
		dockShelf: nothing,
		toggleInspector: nothing,
		showProperties: nothing,
		openPropertiesWindow: nothing,
		resetFolderView: nothing,
	};
}
