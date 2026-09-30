// Exposes typed guest-side wrappers for the system appearance plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ButtonLayout } from './bindings/ButtonLayout';
import type { DesktopEnvironment } from './bindings/DesktopEnvironment';
import type { LayoutSource } from './bindings/LayoutSource';
import type { PluginStatus } from './bindings/PluginStatus';
import type { TitlebarAction } from './bindings/TitlebarAction';
import type { TitlebarActions } from './bindings/TitlebarActions';
import type { TitlebarPreferences } from './bindings/TitlebarPreferences';
import type { TitlebarSnapshot } from './bindings/TitlebarSnapshot';
import type { WindowButton } from './bindings/WindowButton';

const PREFIX = 'plugin:system-appearance|';

/** Event emitted to all windows when the titlebar preferences change. */
export const TITLEBAR_PREFERENCES_CHANGED_EVENT =
	'system-appearance://titlebar-preferences-changed';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports whether the platform's preferences could be read and which sources worked. */
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

/** Subscribes to preference changes and resolves to a function that unsubscribes. */
export function onTitlebarPreferencesChanged(
	callback: (_snapshot: TitlebarSnapshot) => void,
): Promise<() => void> {
	return listen<TitlebarSnapshot>(TITLEBAR_PREFERENCES_CHANGED_EVENT, (event) =>
		callback(event.payload),
	);
}

export type {
	ButtonLayout,
	DesktopEnvironment,
	LayoutSource,
	PluginStatus,
	TitlebarAction,
	TitlebarActions,
	TitlebarPreferences,
	TitlebarSnapshot,
	WindowButton,
};
