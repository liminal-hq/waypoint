// Exposes typed guest-side wrappers for the window manager plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Session } from './bindings/Session';
import type { WindowCapabilities } from './bindings/WindowCapabilities';
import type { WindowPosition } from './bindings/WindowPosition';

const PREFIX = 'plugin:window-manager|';

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

export type { PluginStatus, Session, WindowCapabilities, WindowPosition };
