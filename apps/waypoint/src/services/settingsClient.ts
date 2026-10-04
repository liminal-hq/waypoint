// The application settings as the pages use them: read, change, and hear changes from any window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	ClickMode,
	DefaultView,
	DndSettings,
	DropActionRule,
	GeneralSettings,
	Settings,
	SettingsCommandError,
	SettingsSnapshot,
	StartupMode,
	UiSettings,
} from '@liminal-hq/waypoint-plugin-settings';
import type { Unsubscribe } from './vfsClient';

export type {
	ClickMode,
	DefaultView,
	DndSettings,
	DropActionRule,
	GeneralSettings,
	Settings,
	SettingsCommandError,
	SettingsSnapshot,
	StartupMode,
	UiSettings,
};

/**
 * Everything the pages ask of the settings plugin. Rust owns the document: `set` asks for a change
 * and the answer comes back as the snapshot in force, and as an event to every window (the
 * window that asked included). `FakeSettingsClient` keeps the same contract in memory.
 */
export interface SettingsClient {
	/** The settings in force and their revision. */
	snapshot(): Promise<SettingsSnapshot>;
	/** Rejects with a `SettingsCommandError` and changes nothing for a refused value. */
	set(settings: Settings): Promise<SettingsSnapshot>;
	/**
	 * Changes only the named `ui` settings, on top of whatever is in force in Rust: the call the
	 * main windows make, which cannot turn another window's change to the rest back.
	 */
	setUi(change: Partial<UiSettings>): Promise<SettingsSnapshot>;
	onChanged(listener: (snapshot: SettingsSnapshot) => void): Unsubscribe;
}

/** The settings a window shows before Rust has answered, and everywhere there is no plugin: the defaults (`waypoint-settings`). */
export const DEFAULT_SETTINGS: Settings = {
	general: {
		showHiddenDefault: false,
		startup: 'restoreSession',
		defaultView: 'list',
		clickMode: 'double',
		rememberFolderViews: true,
		gitDecorations: true,
	},
	dnd: {
		defaultActionRule: 'byVolume',
		springLoadMs: 600,
		shelfPersist: true,
	},
	ui: {
		actionBar: true,
		actionBarLabels: true,
		appNameInTitle: false,
		menuBar: false,
		gitColumn: true,
	},
	appearance: {
		mode: 'system',
		themeSource: 'liminal',
		accent: { kind: 'ember' },
		density: 'comfortable',
		iconStyle: 'regular',
		iconTheme: 'waypoint',
		folderColour: 'liminal',
		matchSystemColours: false,
	},
	transparency: {
		enabled: false,
		opacity: 82,
		rowsOpacity: 90,
		sidebarOpacity: 94,
		contentOpacity: 98,
		blur: 'low',
		regions: { sidebar: true, content: false, titleBar: true },
		menus: false,
		menuOpacity: 96,
		solidWhenUnfocused: true,
	},
	accessibility: {
		highContrast: 'follow',
		textSize: 100,
		touchMode: 'auto',
		reducedMotion: 'follow',
		reducedTransparency: 'follow',
		strongFocusRing: false,
	},
	locale: {
		language: 'system',
		direction: 'auto',
	},
	previews: {
		thumbnails: true,
		maxFileMb: 50,
		folderPeeks: true,
		hoverToPeek: false,
		measureHomeOnOpen: true,
	},
	integrations: {
		notifications: false,
		notificationActions: true,
		launcherProgress: false,
		preventSleep: false,
		defaultFileManager: false,
		globalShortcutEnabled: false,
		globalShortcut: null,
		rememberVolumePassphrases: false,
	},
};

/** The ranges and choices Rust enforces for the milestone 5 sections, for the pages that offer them. */
export const OPACITY_MIN = 40;
export const OPACITY_MAX = 100;
export const MENU_OPACITY_MIN = 60;
export const TEXT_SIZES = [100, 115, 130] as const;
export const PREVIEW_MAX_MB_MIN = 1;
export const PREVIEW_MAX_MB_MAX = 2048;
/** The languages with a message catalogue, as BCP 47 tags; `system` follows the OS. */
export const SUPPORTED_LANGUAGES = ['en-CA', 'fr-CA'] as const;
/** The developer-only pseudo-locales, which a debug build of Rust accepts too. */
export const PSEUDO_LANGUAGES = ['en-XA', 'ar-XB'] as const;

/** The spring-load delay's range in milliseconds, which Rust enforces and the page offers. */
export const SPRING_LOAD_MIN_MS = 200;
export const SPRING_LOAD_MAX_MS = 2000;

/** Whether `error` is what a settings command rejects with. */
export function isSettingsError(error: unknown): error is SettingsCommandError {
	return (
		typeof error === 'object' &&
		error !== null &&
		'kind' in error &&
		'message' in error &&
		typeof (error as { message: unknown }).message === 'string'
	);
}
