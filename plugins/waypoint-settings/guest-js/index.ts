// Exposes typed guest-side wrappers for the waypoint-settings plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ClickMode } from '@liminal-hq/waypoint-protocol/generated/ClickMode';
import type { DefaultView } from '@liminal-hq/waypoint-protocol/generated/DefaultView';
import type { DndSettings } from '@liminal-hq/waypoint-protocol/generated/DndSettings';
import type { DropActionRule } from '@liminal-hq/waypoint-protocol/generated/DropActionRule';
import type { FolderView } from '@liminal-hq/waypoint-protocol/generated/FolderView';
import type { FolderViewChange } from '@liminal-hq/waypoint-protocol/generated/FolderViewChange';
import type { FolderViewEntry } from '@liminal-hq/waypoint-protocol/generated/FolderViewEntry';
import type { FolderViewPatch } from '@liminal-hq/waypoint-protocol/generated/FolderViewPatch';
import type { FolderViewsChanged } from '@liminal-hq/waypoint-protocol/generated/FolderViewsChanged';
import type { FolderViewsSnapshot } from '@liminal-hq/waypoint-protocol/generated/FolderViewsSnapshot';
import type { ExportReceipt } from '@liminal-hq/waypoint-protocol/generated/ExportReceipt';
import type { GeneralSettings } from '@liminal-hq/waypoint-protocol/generated/GeneralSettings';
import type { ImportPreview } from '@liminal-hq/waypoint-protocol/generated/ImportPreview';
import type { ListColumnWidths } from '@liminal-hq/waypoint-protocol/generated/ListColumnWidths';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import type { Settings } from '@liminal-hq/waypoint-protocol/generated/Settings';
import type { SettingsSnapshot } from '@liminal-hq/waypoint-protocol/generated/SettingsSnapshot';
import type { StartupMode } from '@liminal-hq/waypoint-protocol/generated/StartupMode';
import type { UiSettings } from '@liminal-hq/waypoint-protocol/generated/UiSettings';

const PREFIX = 'plugin:waypoint-settings|';

/** Sent to every window after every change; the payload is a `SettingsSnapshot`. */
export const SETTINGS_EVENT = 'waypoint-settings://changed';

/**
 * What a rejected command carries. `invalid` is a value outside its range (`field`, `min` and
 * `max` say which); `storage` is settings that could not be saved. Either way nothing changed.
 *
 * The export and import commands add: `transfer`, a file that cannot be imported (`reason` says
 * how: `not-a-bundle`, `corrupt`, `newer-format`, `too-large`, `unsafe`, `invalid` or `nothing`);
 * `apply`, an import that failed part way and was put back; `io`, a file that could not be read or
 * written; `unavailable`, no file dialog on this system; and `stale`, an import that is no longer
 * the one ready to apply.
 */
export interface SettingsCommandError {
	kind: 'invalid' | 'storage' | 'transfer' | 'apply' | 'io' | 'unavailable' | 'stale';
	message: string;
	field?: string;
	min?: number;
	max?: number;
	reason?: string;
}

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports whether the settings plugin works. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** The settings in force and their revision. */
export function getSettings(): Promise<SettingsSnapshot> {
	return cmd<SettingsSnapshot>('get_settings');
}

/**
 * Saves new settings and returns what is in force. Rejects with a `SettingsCommandError` and
 * changes nothing for a value out of range or a save that fails.
 */
export function setSettings(settings: Settings): Promise<SettingsSnapshot> {
	return cmd<SettingsSnapshot>('set_settings', { settings });
}

/** The Action bar choices a window changes; a field left out keeps whatever is in force. */
export interface UiSettingsChange {
	actionBar?: boolean;
	actionBarLabels?: boolean;
	gitColumn?: boolean;
	storageClassColumn?: boolean;
}

/**
 * Changes only the `ui` settings named in `change`, on top of what is in force now (Rust merges
 * them), so a window with an older copy of the document cannot turn other settings back. The
 * main windows may call this; they may not call `setSettings`.
 */
export function setUiSettings(change: UiSettingsChange): Promise<SettingsSnapshot> {
	return cmd<SettingsSnapshot>('set_ui_settings', { change });
}

/**
 * Asks where to save with the system's dialog and writes the settings there: a `.json` when one
 * configuration file is exported and a `.zip` when there are several. `null` when the dialog was
 * closed. `utcOffsetMinutes` (as `-new Date().getTimezoneOffset()`) puts the person's own date in
 * the suggested name. Only the Settings window may call this.
 */
export function exportSettings(utcOffsetMinutes: number): Promise<ExportReceipt | null> {
	return cmd<ExportReceipt | null>('export_settings', { utcOffsetMinutes });
}

/**
 * Asks which file to read with the system's dialog and plans importing it, changing nothing.
 * `null` when the dialog was closed. Only the Settings window may call this.
 */
export function planSettingsImport(): Promise<ImportPreview | null> {
	return cmd<ImportPreview | null>('plan_settings_import');
}

/**
 * Applies the plan just made (all or nothing, as one change every window hears) and returns what
 * is now in force. Rust reads the file again; the page sends only the plan's number. Only the
 * Settings window may call this.
 */
export function applySettingsImport(planId: number): Promise<SettingsSnapshot> {
	return cmd<SettingsSnapshot>('apply_settings_import', { planId });
}

/** Hears every change. Read `getSettings` first and apply snapshots with a higher revision. */
export function onSettingsChanged(
	listener: (snapshot: SettingsSnapshot) => void,
): Promise<UnlistenFn> {
	return listen<SettingsSnapshot>(SETTINGS_EVENT, (e) => listener(e.payload));
}

/** Sent to every window after every change to the remembered folder views; the payload is a `FolderViewsChanged`. */
export const FOLDER_VIEWS_EVENT = 'waypoint-settings://folder-views';

/** Every remembered folder view and the revision they are at. */
export function getFolderViews(): Promise<FolderViewsSnapshot> {
	return cmd<FolderViewsSnapshot>('get_folder_views');
}

/**
 * Remembers the view, sort or grouping chosen for the folder at `key` (its location's `uri`), on
 * top of what it already remembers: a field left `null` is kept as it is. Resolves to the
 * revision in force. Rejects with a `SettingsCommandError` (`invalid` for a location that cannot
 * be one, `storage` for a save that failed) and changes nothing.
 */
export function rememberFolderView(key: string, patch: FolderViewPatch): Promise<number> {
	return cmd<number>('remember_folder_view', { key, patch });
}

/** Makes the folder at `key` forget its own view, so it shows the window's again. Resolves to the revision in force. */
export function resetFolderView(key: string): Promise<number> {
	return cmd<number>('reset_folder_view', { key });
}

/**
 * Hears every change to the remembered views. Read `getFolderViews` first; an event whose
 * revision is not the next one means a change was missed, and the snapshot is read again.
 */
export function onFolderViewsChanged(
	listener: (changed: FolderViewsChanged) => void,
): Promise<UnlistenFn> {
	return listen<FolderViewsChanged>(FOLDER_VIEWS_EVENT, (e) => listener(e.payload));
}

export type {
	FolderView,
	FolderViewChange,
	FolderViewEntry,
	FolderViewPatch,
	FolderViewsChanged,
	FolderViewsSnapshot,
	ClickMode,
	DefaultView,
	DndSettings,
	DropActionRule,
	ExportReceipt,
	GeneralSettings,
	ImportPreview,
	ListColumnWidths,
	PluginStatus,
	Settings,
	SettingsSnapshot,
	StartupMode,
	UiSettings,
};
