// Exposes typed guest-side wrappers for the system appearance plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppearanceFeature } from './bindings/AppearanceFeature';
import type { AppearanceFeatureStatus } from './bindings/AppearanceFeatureStatus';
import type { AppearancePreferences } from './bindings/AppearancePreferences';
import type { AppearanceSource } from './bindings/AppearanceSource';
import type { AppearanceSources } from './bindings/AppearanceSources';
import type { AppearanceValues } from './bindings/AppearanceValues';
import type { ButtonLayout } from './bindings/ButtonLayout';
import type { ColourScheme } from './bindings/ColourScheme';
import type { Contrast } from './bindings/Contrast';
import type { DesktopEnvironment } from './bindings/DesktopEnvironment';
import type { LayoutSource } from './bindings/LayoutSource';
import type { Palette } from './bindings/Palette';
import type { PaletteColour } from './bindings/PaletteColour';
import type { PaletteColours } from './bindings/PaletteColours';
import type { PaletteEntry } from './bindings/PaletteEntry';
import type { PaletteSource } from './bindings/PaletteSource';
import type { PaletteStatus } from './bindings/PaletteStatus';
import type { PluginStatus } from './bindings/PluginStatus';
import type { TitlebarAction } from './bindings/TitlebarAction';
import type { TitlebarActions } from './bindings/TitlebarActions';
import type { TitlebarPreferences } from './bindings/TitlebarPreferences';
import type { TitlebarSnapshot } from './bindings/TitlebarSnapshot';
import type { UnavailableReason } from './bindings/UnavailableReason';
import type { WindowButton } from './bindings/WindowButton';

const PREFIX = 'plugin:system-appearance|';

/** Event emitted to all windows when the titlebar preferences change. */
export const TITLEBAR_PREFERENCES_CHANGED_EVENT =
	'system-appearance://titlebar-preferences-changed';

/** Event emitted to all windows when the appearance preferences change. */
export const APPEARANCE_CHANGED_EVENT = 'system-appearance://appearance-changed';

/** Event emitted to all windows when the colour palette changes. */
export const PALETTE_CHANGED_EVENT = 'system-appearance://palette-changed';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/**
 * Reports whether the platform's preferences could be read and which sources worked. `appearance`
 * lists every appearance feature, with a typed `reason` for each one that does not work here.
 */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/**
 * Reads the current titlebar preferences, stamped with a revision; falls back to a default with
 * source `default`. Keep the highest revision seen and ignore anything older, because a change
 * event and a read can arrive in either order.
 */
export function getTitlebarPreferences(): Promise<TitlebarSnapshot> {
	return cmd<TitlebarSnapshot>('get_titlebar_preferences');
}

/**
 * Reads the current appearance preferences (colour scheme, accent, contrast, reduced motion and
 * transparency, text scale and icon theme), stamped with a revision, and the source that supplied
 * each one. A preference nothing could answer holds a neutral value and has no source; `getStatus`
 * says why. Keep the highest revision seen and ignore anything older.
 */
export function getAppearance(): Promise<AppearancePreferences> {
	return cmd<AppearancePreferences>('get_appearance');
}

/**
 * Reads the operating system's colour palette (window and view backgrounds and text, the raised
 * surface, the selection and its text, the border, the focus colour and the warning, error and
 * success colours), stamped with a revision. Each colour is a `PaletteEntry` with the `source`
 * that supplied it, or a typed `reason` and a `detail` when it is unavailable; `status` says
 * whether the palette can be used at all. Keep the highest revision seen and ignore anything older.
 */
export function getPalette(): Promise<Palette> {
	return cmd<Palette>('get_palette');
}

/** Subscribes to titlebar preference changes and resolves to a function that unsubscribes. */
export function onTitlebarPreferencesChanged(
	callback: (_snapshot: TitlebarSnapshot) => void,
): Promise<() => void> {
	return listen<TitlebarSnapshot>(TITLEBAR_PREFERENCES_CHANGED_EVENT, (event) =>
		callback(event.payload),
	);
}

/**
 * Subscribes to appearance changes and resolves to a function that unsubscribes. Subscribe first
 * and read second, so a change made while starting up is not missed.
 */
export function onAppearanceChanged(
	callback: (_preferences: AppearancePreferences) => void,
): Promise<() => void> {
	return listen<AppearancePreferences>(APPEARANCE_CHANGED_EVENT, (event) =>
		callback(event.payload),
	);
}

/**
 * Subscribes to palette changes and resolves to a function that unsubscribes. Subscribe first and
 * read second, so a change made while starting up is not missed.
 */
export function onPaletteChanged(callback: (_palette: Palette) => void): Promise<() => void> {
	return listen<Palette>(PALETTE_CHANGED_EVENT, (event) => callback(event.payload));
}

export type {
	AppearanceFeature,
	AppearanceFeatureStatus,
	AppearancePreferences,
	AppearanceSource,
	AppearanceSources,
	AppearanceValues,
	ButtonLayout,
	ColourScheme,
	Contrast,
	DesktopEnvironment,
	LayoutSource,
	Palette,
	PaletteColour,
	PaletteColours,
	PaletteEntry,
	PaletteSource,
	PaletteStatus,
	PluginStatus,
	TitlebarAction,
	TitlebarActions,
	TitlebarPreferences,
	TitlebarSnapshot,
	UnavailableReason,
	WindowButton,
};
