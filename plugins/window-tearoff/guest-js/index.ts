// Exposes typed guest-side wrappers for the window tear-off plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { BeginReport } from './bindings/BeginReport';
import type { BeginState } from './bindings/BeginState';
import type { DropReport } from './bindings/DropReport';
import type { Hit } from './bindings/Hit';
import type { Outcome } from './bindings/Outcome';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Point } from './bindings/Point';
import type { Region } from './bindings/Region';
import type { Size } from './bindings/Size';
import type { UnavailableFeature } from './bindings/UnavailableFeature';

const PREFIX = 'plugin:window-tearoff|';

/** Sent to the ghost window with the drag's payload. */
export const PAYLOAD_EVENT = 'window-tearoff://payload';
/** Sent to the window that began a drag when it ran too long and was ended. */
export const TIMEOUT_EVENT = 'window-tearoff://timeout';
/** Sent to the window that began a drag when the cursor value froze (`true`) or moved again (`false`). */
export const CURSOR_STALE_EVENT = 'window-tearoff://cursor-stale';

/** The features `getStatus` can report. */
export type Feature = 'ghost' | 'cursor_follow' | 'window_position' | 'hit_test';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/**
 * Reports which tear-off features work on this system, and why the others do not.
 * The first call probes the windowing system and can take up to a second, so call it once at startup.
 */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** True if `feature` is in `status.features`. */
export function hasFeature(status: PluginStatus, feature: Feature): boolean {
	return status.features.includes(feature);
}

/**
 * Starts a drag: shows the ghost under the cursor with `payload` and follows the cursor until `end`.
 * `grabOffset` is where inside the dragged thing the user grabbed it, in logical pixels from its top-left; `size` is the ghost's logical size.
 * Resolves to `{ state: 'noGhost' }` where no ghost can be shown (render a preview in the page instead), and `{ state: 'alreadyActive' }` if a drag is running.
 */
export function begin(payload: unknown, grabOffset: Point, size: Size): Promise<BeginReport> {
	return cmd<BeginReport>('begin', { payload, grabOffset, size });
}

/** Replaces the ghost's payload while a drag is in progress. */
export function update(payload: unknown): Promise<void> {
	return cmd<void>('update', { payload });
}

/**
 * Ends the drag and hides the ghost.
 * Resolves to where the cursor was (physical pixels) and the registered region it was over; a cancelled drag never has a hit.
 */
export function end(outcome: Outcome): Promise<DropReport> {
	return cmd<DropReport>('end', { outcome });
}

/** Registers the calling window's drop regions, in logical pixels from the top-left of its content, replacing its earlier ones. */
export function setDropRegions(regions: Region[]): Promise<void> {
	return cmd<void>('set_drop_regions', { regions });
}

/** The native cursor in physical pixels, or null where the system does not report a usable one. */
export function getCursor(): Promise<Point | null> {
	return cmd<Point | null>('get_cursor');
}

/**
 * The registered region under the cursor right now, without ending the drag, so a caller can say what a release would do.
 * Null where the system reports no usable cursor, cannot hit-test, or the cursor is over no region.
 */
export function hitTest(): Promise<Hit | null> {
	return cmd<Hit | null>('hit_test');
}

/** The payload of the drag in progress, so a ghost page that loaded after `begin` can still draw it. */
export function getPayload<T = unknown>(): Promise<T | null> {
	return cmd<T | null>('get_payload');
}

/** Listens, in the ghost window, for the drag's payload as it is sent and updated; `null` means the drag ended and the card should clear. */
export function onPayload<T = unknown>(handler: (_payload: T) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<T>(PAYLOAD_EVENT, (event) => handler(event.payload));
}

/** Listens for a drag the plugin ended because it ran too long. */
export function onTimeout(handler: () => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen(TIMEOUT_EVENT, () => handler());
}

/** Listens for the cursor value freezing while a button is held (`true`) and moving again (`false`). */
export function onCursorStale(handler: (_stale: boolean) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<boolean>(CURSOR_STALE_EVENT, (event) =>
		handler(event.payload),
	);
}

export type {
	BeginReport,
	BeginState,
	DropReport,
	Hit,
	Outcome,
	PluginStatus,
	Point,
	Region,
	Size,
	UnavailableFeature,
};
