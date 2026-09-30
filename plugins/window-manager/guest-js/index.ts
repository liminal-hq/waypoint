// Exposes typed guest-side wrappers for the window manager plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Session } from './bindings/Session';
import type { WindowCapabilities } from './bindings/WindowCapabilities';
import type { WindowPosition } from './bindings/WindowPosition';

const PREFIX = 'plugin:window-manager|';
const ALWAYS_ON_TOP_CHANGED_EVENT = 'window-manager://always-on-top-changed';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports whether any window manager feature works on this system, and which. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** Reports the windowing system and which window manager features the app can use. */
export function getCapabilities(): Promise<WindowCapabilities> {
	return cmd<WindowCapabilities>('get_capabilities');
}

/**
 * Asks the compositor to show its own window menu for the calling window at `position` (CSS pixels from the window's top-left).
 * Resolves to true if the compositor accepted the request, and false if it is unsupported or was refused. Wayland only honours it right after a real mouse press, so call it from a pointer event handler.
 */
export function showSystemWindowMenu(position: WindowPosition): Promise<boolean> {
	return cmd<boolean>('show_system_window_menu', { position });
}

/**
 * Whether the window manager is keeping the calling window above others, read from the window manager rather than echoed from the last request.
 * Resolves to null where it cannot be observed: Wayland has no such state, and unsupported targets have no window manager integration.
 */
export function getAlwaysOnTop(): Promise<boolean | null> {
	return cmd<boolean | null>('get_always_on_top');
}

/**
 * Listens for the window manager changing whether the calling window is kept above others, including through its own window menu.
 * Only X11 reports these changes. Subscribe first and then call `getAlwaysOnTop()`, so a change between the two cannot be missed.
 */
export function onAlwaysOnTopChanged(handler: (alwaysOnTop: boolean) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<boolean>(ALWAYS_ON_TOP_CHANGED_EVENT, (event) =>
		handler(event.payload),
	);
}

export type { PluginStatus, Session, WindowCapabilities, WindowPosition };
