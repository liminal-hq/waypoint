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
	},
	dnd: {
		defaultActionRule: 'byVolume',
		springLoadMs: 600,
		shelfPersist: true,
	},
	ui: {
		actionBar: true,
		actionBarLabels: true,
		appMenuLabel: false,
	},
};

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
