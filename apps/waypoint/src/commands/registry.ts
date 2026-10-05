// The command registry: one typed table of the app's commands that the menu, the Action bar and the palette all list
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupBy } from '@liminal-hq/waypoint-protocol/generated/GroupBy';
import type { PlaceKind } from '@liminal-hq/waypoint-protocol/generated/PlaceKind';
import type { SortKey } from '@liminal-hq/waypoint-protocol/generated/SortKey';
import type { ComponentType } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import type { IconProps } from '../icons/AppIcons';
import {
	FolderTabIcon,
	GridViewIcon,
	HomeIcon,
	ListViewIcon,
	PinIcon,
	SidebarIcon,
} from '../icons/AppIcons';
import {
	ActionBarIcon,
	ArrowDownIcon,
	ClockIcon,
	GitIcon,
	ColumnsIcon,
	CommandPaletteIcon,
	CopyIcon,
	CopyToIcon,
	CutIcon,
	DeleteForeverIcon,
	DuplicateIcon,
	EditIcon,
	EyeIcon,
	FolderOpenIcon,
	HelpIcon,
	InfoIcon,
	InvertSelectionIcon,
	KeyboardIcon,
	LinkIcon,
	MoveToIcon,
	NewFileIcon,
	UngroupIcon,
	NewFolderIcon,
	NewTabIcon,
	PauseIcon,
	PlayIcon,
	PasteIcon,
	RedoIcon,
	RestoreIcon,
	SelectAllIcon,
	SettingsIcon,
	SizeIcon,
	TagIcon,
	TextIcon,
	TourIcon,
	TrashIcon,
	UndoIcon,
	WindowIcon,
	CloseOthersIcon,
} from '../icons/MenuIcons';
import { OpenWithIcon } from '../openWith/OpenWithIcons';
import { OverviewIcon } from '../overview/OverviewIcons';
import { InspectorIcon, PropertiesIcon } from '../inspector/InspectorIcons';
import { ServerIcon } from '../connections/ConnectionIcons';
import { AddToShelfIcon, ShelfIcon } from '../shelf/ShelfIcons';
import type { CommandActions, CommandFacts } from './commandEnv';
import type { FileCommandId } from '../ops/fileCommands';

/** Where a command lives in the menus; the palette groups by it too. */
export type CommandGroup = 'file' | 'edit' | 'view' | 'go' | 'tabs' | 'window' | 'app';

/** The order the palette lists groups in when scores tie. */
export const GROUP_ORDER: readonly CommandGroup[] = [
	'file',
	'edit',
	'view',
	'go',
	'tabs',
	'window',
	'app',
];

export type CommandId =
	| Exclude<FileCommandId, 'pasteInto'>
	| 'batchRename'
	| 'openWith'
	// The Trash
	| 'restoreFromTrash'
	| 'deleteFromTrash'
	| 'selectAll'
	| 'invertSelection'
	| 'sortName'
	| 'sortSize'
	| 'sortModified'
	| 'sortKind'
	| 'sortDeleted'
	| 'sortGit'
	| 'sortDescending'
	| 'sortFoldersFirst'
	| 'groupNone'
	| 'groupKind'
	| 'groupModified'
	| 'groupSize'
	| 'groupName'
	| 'groupType'
	| 'newWindow'
	| 'newTab'
	| 'connectToServer'
	| 'closeTab'
	| 'reopenClosedTab'
	| 'duplicateTab'
	| 'splitView'
	| 'moveTabToNewWindow'
	| 'viewList'
	| 'viewGrid'
	| 'resetFolderView'
	| 'showHidden'
	| 'sidebar'
	| 'actionBar'
	| 'alwaysOnTop'
	| 'closeWindow'
	| 'pauseAll'
	| 'resumeAll'
	| 'settings'
	| 'commandPalette'
	// Help
	| 'help'
	| 'keyboardShortcuts'
	| 'tour'
	| 'about'
	| 'linkTo'
	| GoCommandId
	// The Shelf
	| 'toggleShelf'
	| 'addToShelf'
	| 'focusShelf'
	| 'undockShelf'
	| 'dockShelf'
	// The Inspector
	| 'toggleInspector'
	| 'showProperties'
	| 'propertiesInWindow';

/** The commands that open a place of the sidebar. */
export type GoCommandId =
	| 'goHome'
	| 'goDesktop'
	| 'goDocuments'
	| 'goDownloads'
	| 'goPictures'
	| 'goMusic'
	| 'goVideos'
	| 'goTrash'
	| 'openOverview';

/** Whether a command is offered now, and if it is offered but cannot run, why. */
export type Availability =
	| { visible: false; enabled: false }
	| { visible: true; enabled: true }
	| { visible: true; enabled: false; reason: MessageId };

export const SHOWN: Availability = { visible: true, enabled: true };
export const HIDDEN: Availability = { visible: false, enabled: false };

/** Offered but not runnable: the row stays so the command can be found, and `reason` says what is missing. */
export function blocked(reason: MessageId): Availability {
	return { visible: true, enabled: false, reason };
}

export interface CommandDef {
	id: CommandId;
	/** The label's message. */
	label: MessageId;
	/** A label that names what the command will do now ("Undo Move 3 items to Trash"); `label` when it returns `null`. */
	describe?: (facts: CommandFacts) => string | null;
	/** The key the window binds, as displayed ("Ctrl+Shift+Z"); absent when nothing is bound. */
	shortcut?: string;
	icon?: ComponentType<IconProps>;
	group: CommandGroup;
	/** Whether the command is offered, and whether it can run now. Pure over the facts. */
	when(facts: CommandFacts): Availability;
	/** For a command that is a setting, whether it is on. */
	checked?: (facts: CommandFacts) => boolean;
	/** Does it. Called only when `when` says it can run. */
	run(actions: CommandActions, facts: CommandFacts): void;
}

/** A command as a list shows it: the definition resolved against the facts. */
export interface CommandView {
	id: CommandId;
	label: string;
	shortcut: string | undefined;
	icon: ComponentType<IconProps> | undefined;
	group: CommandGroup;
	visible: boolean;
	enabled: boolean;
	/** Why it is disabled; absent when it is enabled or hidden. */
	reason: string | undefined;
	/** Present for a command that is a setting. */
	checked: boolean | undefined;
}

// --- Availability rules ------------------------------------------------------------------

/** What a file command's availability is, from `commandStates`, with the reason it would give. */
function fileCommand(
	id: FileCommandId,
	reason: (facts: CommandFacts) => MessageId,
): CommandDef['when'] {
	return (facts) => {
		const state = facts.file[id];
		if (!state.visible) return HIDDEN;
		return state.enabled ? SHOWN : blocked(reason(facts));
	};
}

const needsSelection = () => 'cmd.reason.nothingSelected' as const;

const needsOtherPane = (facts: CommandFacts) =>
	facts.selected === 0 ? 'cmd.reason.nothingSelected' : 'cmd.reason.otherPaneReadOnly';

function needs(has: (facts: CommandFacts) => boolean, reason: MessageId): CommandDef['when'] {
	return (facts) => (has(facts) ? SHOWN : blocked(reason));
}

/** The sort keys each kind of listing offers (the Trash has no modified time or kind, only when it was deleted). */
const SORT_KEYS: Record<SortKey, 'all' | 'trash' | 'folder' | 'git'> = {
	name: 'all',
	size: 'all',
	modified: 'folder',
	kind: 'folder',
	deleted: 'trash',
	// Only in a folder of a working tree, where Git has something to say.
	git: 'git',
};

function sortByKey(
	key: SortKey,
	id: CommandId,
	label: MessageId,
	icon: ComponentType<IconProps>,
): CommandDef {
	return {
		id,
		label,
		icon,
		group: 'view',
		when: (facts) => {
			if (!facts.sort) return HIDDEN;
			const offered = SORT_KEYS[key];
			if (offered === 'git') return facts.git && !facts.trash ? SHOWN : HIDDEN;
			return offered === 'all' || (offered === 'trash') === facts.trash ? SHOWN : HIDDEN;
		},
		checked: (facts) => facts.sort?.key === key,
		// The active key stays as it is (Descending is its own command); a new key starts ascending.
		run: (actions) =>
			actions.changeSort((sort) => (sort.key === key ? sort : { ...sort, key, descending: false })),
	};
}

/** "Group by Kind": divides the listing into headed groups, offered where there is a listing that can be grouped (not the Trash). */
function groupBy(
	by: GroupBy,
	id: CommandId,
	label: MessageId,
	icon: ComponentType<IconProps>,
): CommandDef {
	return {
		id,
		label,
		icon,
		group: 'view',
		when: (facts) => (facts.sort && !facts.trash ? SHOWN : HIDDEN),
		checked: (facts) => facts.sort?.groupBy === by,
		run: (actions) =>
			actions.changeSort((sort) => (sort.groupBy === by ? sort : { ...sort, groupBy: by })),
	};
}

/** "Go to Downloads": opens one of the sidebar's places, offered where the sidebar has it. */
function goToPlace(
	place: PlaceKind,
	id: GoCommandId,
	label: MessageId,
	icon: ComponentType<IconProps>,
): CommandDef {
	return {
		id,
		label,
		icon,
		group: 'go',
		when: (facts) =>
			!facts.places.includes(place) ? HIDDEN : facts.tab ? SHOWN : blocked('cmd.reason.noTab'),
		run: (actions) => actions.goToPlace(place),
	};
}

// --- The table -----------------------------------------------------------------------------

export const COMMANDS: readonly CommandDef[] = [
	// File
	{
		id: 'newWindow',
		label: 'cmd.newWindow',
		shortcut: 'Ctrl+Shift+N',
		icon: WindowIcon,
		group: 'file',
		when: () => SHOWN,
		run: (a) => a.newWindow(),
	},
	{
		id: 'newTab',
		label: 'cmd.newTab',
		shortcut: 'Ctrl+T',
		icon: NewTabIcon,
		group: 'file',
		when: () => SHOWN,
		run: (a) => a.newTab(),
	},
	{
		id: 'connectToServer',
		label: 'cmd.connectToServer',
		icon: ServerIcon,
		group: 'file',
		when: (f) => (f.connections ? SHOWN : HIDDEN),
		run: (a) => a.connectToServer(),
	},
	{
		id: 'newFolder',
		label: 'cmd.newFolder',
		shortcut: 'F7',
		icon: NewFolderIcon,
		group: 'file',
		when: fileCommand('newFolder', () => 'cmd.reason.noListing'),
		run: (a) => void a.files?.newFolder(),
	},
	{
		id: 'newFile',
		label: 'cmd.newFile',
		shortcut: 'Shift+F7',
		icon: NewFileIcon,
		group: 'file',
		when: fileCommand('newFile', () => 'cmd.reason.noListing'),
		run: (a) => void a.files?.newFile(),
	},
	{
		id: 'rename',
		label: 'menu.rename',
		shortcut: 'F2',
		icon: EditIcon,
		group: 'file',
		when: fileCommand('rename', () => 'cmd.reason.nothingFocused'),
		run: (a) => a.files?.rename(),
	},
	{
		id: 'openWith',
		label: 'cmd.openWith',
		icon: OpenWithIcon,
		group: 'file',
		// Where the plugin can do it, for files on this computer; it opens on the selection.
		when: (f) =>
			!f.listing || f.trash || !f.local || !f.openWith
				? HIDDEN
				: f.selected > 0
					? SHOWN
					: blocked('cmd.reason.nothingSelected'),
		run: (a) => a.openWith(),
	},
	{
		id: 'batchRename',
		label: 'cmd.batchRename',
		shortcut: 'Ctrl+F2',
		icon: EditIcon,
		group: 'file',
		// Offered wherever Rename is (a listing that can be written to); it needs a selection to open on.
		when: (facts) =>
			!facts.file.rename.visible
				? HIDDEN
				: facts.batchRename
					? SHOWN
					: blocked('cmd.reason.nothingSelected'),
		run: (a) => a.batchRename(),
	},
	{
		id: 'duplicate',
		label: 'menu.duplicate',
		shortcut: 'Ctrl+Shift+D',
		icon: DuplicateIcon,
		group: 'file',
		when: fileCommand('duplicate', needsSelection),
		run: (a) => void a.files?.duplicate(),
	},
	{
		id: 'moveToTrash',
		label: 'menu.moveToTrash',
		shortcut: 'Delete',
		icon: TrashIcon,
		group: 'file',
		when: fileCommand('moveToTrash', needsSelection),
		run: (a) => void a.files?.moveToTrash(),
	},
	{
		id: 'deletePermanently',
		label: 'menu.deletePermanently',
		shortcut: 'Shift+Delete',
		icon: DeleteForeverIcon,
		group: 'file',
		when: fileCommand('deletePermanently', needsSelection),
		run: (a) => void a.files?.deletePermanently(),
	},
	{
		id: 'restoreFromTrash',
		label: 'trash.restore',
		shortcut: 'Ctrl+Shift+R',
		icon: RestoreIcon,
		group: 'file',
		// Only the Trash lists items to restore; its Delete is "Delete Permanently", with its question.
		when: (facts) =>
			!facts.trash ? HIDDEN : facts.selected > 0 ? SHOWN : blocked('cmd.reason.nothingSelected'),
		run: (a) => a.restoreFromTrash(),
	},
	{
		id: 'deleteFromTrash',
		label: 'trash.delete',
		// Delete is bound here too, but `moveToTrash` shows it: only one command lists a key.
		icon: DeleteForeverIcon,
		group: 'file',
		when: (facts) =>
			!facts.trash ? HIDDEN : facts.selected > 0 ? SHOWN : blocked('cmd.reason.nothingSelected'),
		run: (a) => a.deleteFromTrash(),
	},
	{
		id: 'closeTab',
		label: 'tabs.menu.close',
		shortcut: 'Ctrl+W',
		icon: CloseOthersIcon,
		group: 'file',
		when: needs((f) => f.tab, 'cmd.reason.noTab'),
		run: (a) => a.closeTab(),
	},
	{
		id: 'pauseAll',
		label: 'cmd.pauseAll',
		icon: PauseIcon,
		group: 'file',
		// Offered wherever there is a queue; it can run when something runs and Pause all is not in force.
		when: (f) => (f.opsRunning > 0 && !f.opsPaused ? SHOWN : blocked('cmd.reason.nothingToPause')),
		run: (a) => a.pauseAll(),
	},
	{
		id: 'resumeAll',
		label: 'cmd.resumeAll',
		icon: PlayIcon,
		group: 'file',
		when: (f) => (f.opsPaused ? SHOWN : blocked('cmd.reason.nothingToResume')),
		run: (a) => a.resumeAll(),
	},
	{
		id: 'closeWindow',
		label: 'cmd.closeWindow',
		icon: WindowIcon,
		group: 'file',
		when: () => SHOWN,
		run: (a) => a.closeWindow(),
	},

	// Edit
	{
		id: 'undo',
		label: 'menu.undo',
		describe: (f) => (f.undoLabel ? tf('menu.undoNamed', { label: f.undoLabel }) : null),
		shortcut: 'Ctrl+Z',
		icon: UndoIcon,
		group: 'edit',
		when: fileCommand('undo', () => 'cmd.reason.nothingToUndo'),
		run: (a) => void a.files?.undo(),
	},
	{
		id: 'redo',
		label: 'menu.redo',
		describe: (f) => (f.redoLabel ? tf('menu.redoNamed', { label: f.redoLabel }) : null),
		shortcut: 'Ctrl+Shift+Z',
		icon: RedoIcon,
		group: 'edit',
		when: fileCommand('redo', () => 'cmd.reason.nothingToRedo'),
		run: (a) => void a.files?.redo(),
	},
	{
		id: 'cut',
		label: 'menu.cut',
		shortcut: 'Ctrl+X',
		icon: CutIcon,
		group: 'edit',
		when: fileCommand('cut', needsSelection),
		run: (a) => void a.files?.cut(),
	},
	{
		id: 'copy',
		label: 'menu.copy',
		shortcut: 'Ctrl+C',
		icon: CopyIcon,
		group: 'edit',
		when: fileCommand('copy', needsSelection),
		run: (a) => void a.files?.copy(),
	},
	{
		id: 'paste',
		label: 'menu.paste',
		shortcut: 'Ctrl+V',
		icon: PasteIcon,
		group: 'edit',
		when: fileCommand('paste', () => 'cmd.reason.clipboardEmpty'),
		run: (a) => void a.files?.paste(),
	},
	{
		id: 'copyTo',
		label: 'menu.copyTo',
		icon: CopyToIcon,
		group: 'edit',
		when: fileCommand('copyTo', needsSelection),
		run: (a) => void a.files?.copyTo(),
	},
	{
		id: 'moveTo',
		label: 'menu.moveTo',
		icon: MoveToIcon,
		group: 'edit',
		when: fileCommand('moveTo', needsSelection),
		run: (a) => void a.files?.moveTo(),
	},
	{
		id: 'linkTo',
		label: 'cmd.linkTo',
		icon: LinkIcon,
		group: 'edit',
		// Links point at local items, and making one on Windows needs a privilege the engine only
		// reports per item, so the command is offered where a link can be relied on to work.
		when: (f) =>
			!f.file.copyTo.visible || !f.local || !f.linkSupported
				? HIDDEN
				: f.selected > 0
					? SHOWN
					: blocked('cmd.reason.nothingSelected'),
		run: (a) => void a.files?.linkTo(),
	},
	{
		id: 'copyToOtherPane',
		label: 'menu.copyToOtherPane',
		shortcut: 'F5',
		icon: CopyToIcon,
		group: 'edit',
		when: fileCommand('copyToOtherPane', needsOtherPane),
		run: (a) => void a.files?.copyToOtherPane(),
	},
	{
		id: 'moveToOtherPane',
		label: 'menu.moveToOtherPane',
		shortcut: 'Shift+F5',
		icon: MoveToIcon,
		group: 'edit',
		when: fileCommand('moveToOtherPane', needsOtherPane),
		run: (a) => void a.files?.moveToOtherPane(),
	},
	{
		id: 'selectAll',
		label: 'cmd.selectAll',
		shortcut: 'Ctrl+A',
		icon: SelectAllIcon,
		group: 'edit',
		when: (f) => (f.listing ? SHOWN : HIDDEN),
		run: (a) => a.selectAll(),
	},
	{
		id: 'invertSelection',
		label: 'cmd.invertSelection',
		shortcut: 'Ctrl+I',
		icon: InvertSelectionIcon,
		group: 'edit',
		when: (f) => (f.listing ? SHOWN : HIDDEN),
		run: (a) => a.invertSelection(),
	},

	// View
	{
		id: 'viewList',
		label: 'view.list',
		shortcut: 'Ctrl+2',
		icon: ListViewIcon,
		group: 'view',
		when: () => SHOWN,
		checked: (f) => f.viewMode === 'list',
		run: (a) => a.setViewMode('list'),
	},
	{
		id: 'viewGrid',
		label: 'view.grid',
		shortcut: 'Ctrl+1',
		icon: GridViewIcon,
		group: 'view',
		when: () => SHOWN,
		checked: (f) => f.viewMode === 'grid',
		run: (a) => a.setViewMode('grid'),
	},
	{
		id: 'resetFolderView',
		label: 'cmd.resetFolderView',
		icon: RestoreIcon,
		group: 'view',
		// Offered wherever a folder could remember a view; runnable once it has.
		when: (f) =>
			f.folderView === 'unavailable'
				? HIDDEN
				: f.folderView === 'remembered'
					? SHOWN
					: blocked('cmd.reason.folderViewDefault'),
		run: (a) => a.resetFolderView(),
	},
	{
		id: 'showHidden',
		label: 'menu.showHidden',
		shortcut: 'Ctrl+H',
		icon: EyeIcon,
		group: 'view',
		// Nothing is hidden in the Trash.
		when: (f) => (f.trash ? HIDDEN : SHOWN),
		checked: (f) => f.showHidden,
		run: (a) => a.toggleHidden(),
	},
	{
		id: 'sidebar',
		label: 'cmd.sidebar',
		shortcut: 'F9',
		icon: SidebarIcon,
		group: 'view',
		when: () => SHOWN,
		checked: (f) => f.sidebarOpen,
		run: (a) => a.toggleSidebar(),
	},
	{
		id: 'actionBar',
		label: 'cmd.actionBar',
		icon: ActionBarIcon,
		group: 'view',
		when: () => SHOWN,
		checked: (f) => f.actionBar,
		run: (a, f) => a.setActionBar(!f.actionBar),
	},
	{
		id: 'splitView',
		label: 'cmd.splitView',
		shortcut: 'F3',
		icon: ColumnsIcon,
		group: 'view',
		when: needs((f) => f.tab, 'cmd.reason.noTab'),
		checked: (f) => f.paired,
		run: (a) => a.toggleSplit(),
	},
	sortByKey('name', 'sortName', 'menu.sort.name', TextIcon),
	sortByKey('size', 'sortSize', 'menu.sort.size', SizeIcon),
	sortByKey('modified', 'sortModified', 'menu.sort.modified', ClockIcon),
	sortByKey('kind', 'sortKind', 'menu.sort.kind', TagIcon),
	sortByKey('deleted', 'sortDeleted', 'menu.sort.deleted', ClockIcon),
	sortByKey('git', 'sortGit', 'menu.sort.git', GitIcon),
	{
		id: 'sortDescending',
		label: 'menu.sort.descending',
		icon: ArrowDownIcon,
		group: 'view',
		when: (f) => (f.sort ? SHOWN : HIDDEN),
		checked: (f) => f.sort?.descending === true,
		run: (a) => a.changeSort((sort) => ({ ...sort, descending: !sort.descending })),
	},
	{
		id: 'sortFoldersFirst',
		label: 'menu.sort.foldersFirst',
		icon: FolderTabIcon,
		group: 'view',
		when: (f) => (f.sort ? SHOWN : HIDDEN),
		checked: (f) => f.sort?.directoriesFirst === true,
		run: (a) => a.changeSort((sort) => ({ ...sort, directoriesFirst: !sort.directoriesFirst })),
	},

	groupBy('none', 'groupNone', 'cmd.group.none', UngroupIcon),
	groupBy('kind', 'groupKind', 'cmd.group.kind', TagIcon),
	groupBy('modified', 'groupModified', 'cmd.group.modified', ClockIcon),
	groupBy('size', 'groupSize', 'cmd.group.size', SizeIcon),
	groupBy('name', 'groupName', 'cmd.group.name', TextIcon),
	groupBy('type', 'groupType', 'cmd.group.type', NewFileIcon),

	// Go
	goToPlace('home', 'goHome', 'cmd.goTo.home', HomeIcon),
	goToPlace('desktop', 'goDesktop', 'cmd.goTo.desktop', FolderOpenIcon),
	goToPlace('documents', 'goDocuments', 'cmd.goTo.documents', FolderOpenIcon),
	goToPlace('downloads', 'goDownloads', 'cmd.goTo.downloads', FolderOpenIcon),
	goToPlace('pictures', 'goPictures', 'cmd.goTo.pictures', FolderOpenIcon),
	goToPlace('music', 'goMusic', 'cmd.goTo.music', FolderOpenIcon),
	goToPlace('videos', 'goVideos', 'cmd.goTo.videos', FolderOpenIcon),
	goToPlace('trash', 'goTrash', 'cmd.goTo.trash', TrashIcon),
	// Overview is a place that opens as the tab's content, so the palette says "Open".
	goToPlace('overview', 'openOverview', 'cmd.openOverview', OverviewIcon),

	// Tabs
	{
		id: 'reopenClosedTab',
		label: 'tabs.menu.reopen',
		shortcut: 'Ctrl+Shift+T',
		icon: RestoreIcon,
		group: 'tabs',
		// Closing a tab sends no event for the closed list, so the window cannot know the list is empty;
		// with nothing to reopen the command says so.
		when: () => SHOWN,
		run: (a) => a.reopenClosedTab(),
	},
	{
		id: 'duplicateTab',
		label: 'tabs.menu.duplicate',
		icon: DuplicateIcon,
		group: 'tabs',
		when: needs((f) => f.tab, 'cmd.reason.noTab'),
		run: (a) => a.duplicateTab(),
	},
	{
		id: 'moveTabToNewWindow',
		label: 'tabs.menu.moveToNewWindow',
		icon: FolderOpenIcon,
		group: 'tabs',
		when: needs((f) => f.tab, 'cmd.reason.noTab'),
		run: (a) => a.moveTabToNewWindow(),
	},

	// Window and app
	{
		id: 'alwaysOnTop',
		label: 'cmd.alwaysOnTop',
		icon: PinIcon,
		group: 'window',
		// Offered only where the window manager can do it (not on Wayland, which has no protocol for it).
		when: (f) => (f.alwaysOnTop.supported ? SHOWN : HIDDEN),
		checked: (f) => f.alwaysOnTop.on,
		run: (a, f) => a.setAlwaysOnTop(!f.alwaysOnTop.on),
	},
	{
		id: 'settings',
		label: 'cmd.settings',
		shortcut: 'Ctrl+,',
		icon: SettingsIcon,
		group: 'app',
		when: () => SHOWN,
		run: (a) => a.openSettings(),
	},
	{
		id: 'commandPalette',
		label: 'cmd.commandPalette',
		shortcut: 'Ctrl+Shift+P',
		icon: CommandPaletteIcon,
		group: 'app',
		when: () => SHOWN,
		run: (a) => a.openPalette(),
	},

	// Help
	{
		id: 'help',
		label: 'cmd.help',
		shortcut: 'F1',
		icon: HelpIcon,
		group: 'app',
		when: () => SHOWN,
		run: (a) => a.openHelp('help'),
	},
	{
		id: 'keyboardShortcuts',
		label: 'cmd.keyboardShortcuts',
		shortcut: '?',
		icon: KeyboardIcon,
		group: 'app',
		when: () => SHOWN,
		run: (a) => a.openHelp('shortcuts'),
	},
	{
		id: 'tour',
		label: 'cmd.tour',
		icon: TourIcon,
		group: 'app',
		when: () => SHOWN,
		run: (a) => a.openHelp('tour'),
	},
	{
		id: 'about',
		label: 'cmd.about',
		icon: InfoIcon,
		group: 'app',
		when: () => SHOWN,
		run: (a) => a.openHelp('about'),
	},

	// The Shelf
	{
		id: 'toggleShelf',
		label: 'cmd.shelf',
		shortcut: 'Ctrl+B',
		icon: ShelfIcon,
		group: 'view',
		when: () => SHOWN,
		checked: (f) => f.shelfOpen,
		run: (a) => a.toggleShelf(),
	},
	{
		id: 'addToShelf',
		label: 'cmd.addToShelf',
		icon: AddToShelfIcon,
		group: 'edit',
		// References only: it reads the selection, so any listing that is not the Trash can offer it.
		when: (f) =>
			!f.listing || f.trash
				? HIDDEN
				: f.selected > 0
					? SHOWN
					: blocked('cmd.reason.nothingSelected'),
		run: (a) => a.addToShelf(),
	},
	{
		id: 'focusShelf',
		label: 'cmd.focusShelf',
		icon: ShelfIcon,
		group: 'view',
		when: () => SHOWN,
		run: (a) => a.focusShelf(),
	},
	{
		id: 'undockShelf',
		label: 'cmd.undockShelf',
		icon: ShelfIcon,
		group: 'view',
		when: (f) => (f.shelfUndocked ? HIDDEN : SHOWN),
		run: (a) => a.undockShelf(),
	},
	{
		id: 'dockShelf',
		label: 'cmd.dockShelf',
		icon: ShelfIcon,
		group: 'view',
		when: (f) => (f.shelfUndocked ? SHOWN : HIDDEN),
		run: (a) => a.dockShelf(),
	},

	// The Inspector
	{
		id: 'toggleInspector',
		label: 'cmd.inspector',
		shortcut: 'F11',
		icon: InspectorIcon,
		group: 'view',
		when: () => SHOWN,
		checked: (f) => f.inspectorOpen,
		run: (a) => a.toggleInspector(),
	},
	{
		id: 'showProperties',
		label: 'cmd.properties',
		icon: PropertiesIcon,
		group: 'view',
		// The Trash's items have no details to read, so the tab is not offered there.
		when: (f) => (!f.listing || f.trash ? HIDDEN : SHOWN),
		run: (a) => a.showProperties(),
	},
	{
		id: 'propertiesInWindow',
		label: 'cmd.propertiesInWindow',
		shortcut: 'Alt+Enter',
		icon: PropertiesIcon,
		group: 'view',
		// A window is about one thing: one item, or the folder when nothing is selected. Not in the
		// Trash, whose items have no details to read, and not where the service does not exist.
		when: (f) =>
			!f.listing || f.trash || !f.propertiesWindow
				? HIDDEN
				: f.selected > 1
					? blocked('cmd.reason.selectOne')
					: SHOWN,
		run: (a) => a.openPropertiesWindow(),
	},
];

const BY_ID: ReadonlyMap<CommandId, CommandDef> = new Map(
	COMMANDS.map((command) => [command.id, command] as const),
);

export function commandDef(id: CommandId): CommandDef {
	const def = BY_ID.get(id);
	if (!def) throw new Error(`unknown command ${id}`);
	return def;
}

/** Resolves one command against the facts: its words, its availability and its state. */
export function viewOf(def: CommandDef, facts: CommandFacts): CommandView {
	const availability = def.when(facts);
	return {
		id: def.id,
		label: def.describe?.(facts) ?? t(def.label),
		shortcut: def.shortcut,
		icon: def.icon,
		group: def.group,
		visible: availability.visible,
		enabled: availability.enabled,
		reason: 'reason' in availability ? t(availability.reason) : undefined,
		checked: def.checked?.(facts),
	};
}

/** Every command, in the registry's order, resolved against `facts`. */
export function evaluateCommands(facts: CommandFacts): CommandView[] {
	return COMMANDS.map((def) => viewOf(def, facts));
}

/**
 * Runs a command if it is offered and enabled, and says whether it ran. A caller that lists
 * commands (a menu, the palette) can pass anything it shows without checking again.
 */
export function runCommand(id: CommandId, actions: CommandActions, facts: CommandFacts): boolean {
	const def = commandDef(id);
	const availability = def.when(facts);
	if (!availability.visible || !availability.enabled) return false;
	def.run(actions, facts);
	return true;
}
