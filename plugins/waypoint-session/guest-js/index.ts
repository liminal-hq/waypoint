// Exposes typed guest-side wrappers for the waypoint-session plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { PluginStatus } from '@liminal-hq/waypoint-protocol/generated/PluginStatus';
import type { SessionEvent } from '@liminal-hq/waypoint-protocol/generated/SessionEvent';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';

const PREFIX = 'plugin:waypoint-session|';
const EVENT = 'waypoint-session://event';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports whether the session plugin works and which features it offers. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** The calling window's whole session at its current revision. */
export function getSnapshot(): Promise<SessionSnapshot> {
	return cmd<SessionSnapshot>('get_snapshot');
}

/** Opens a tab after `after` (or at the end) and returns its id. */
export function openTab(location: Location, after?: TabId, activate = true): Promise<TabId> {
	return cmd<TabId>('open_tab', { location, after: after ?? null, activate });
}

export function closeTab(tab: TabId): Promise<void> {
	return cmd<void>('close_tab', { tab });
}

export function activateTab(tab: TabId): Promise<void> {
	return cmd<void>('activate_tab', { tab });
}

export function moveTab(tab: TabId, index: number): Promise<void> {
	return cmd<void>('move_tab', { tab, index });
}

/** Goes somewhere new in a tab: the current location joins its back history. */
export function navigate(tab: TabId, location: Location): Promise<void> {
	return cmd<void>('navigate', { tab, location });
}

export function back(tab: TabId): Promise<void> {
	return cmd<void>('back', { tab });
}

export function forward(tab: TabId): Promise<void> {
	return cmd<void>('forward', { tab });
}

/** Follows every change to the calling window's session. */
export function onTabsEvent(listener: (event: SessionEvent) => void): Promise<UnlistenFn> {
	return listen<SessionEvent>(EVENT, (e) => listener(e.payload));
}
