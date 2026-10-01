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
import type { PayloadDropped } from './bindings/PayloadDropped';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Point } from './bindings/Point';
import type { Region } from './bindings/Region';
import type { Size } from './bindings/Size';
import type { ToplevelBeginReport } from './bindings/ToplevelBeginReport';
import type { ToplevelBeginState } from './bindings/ToplevelBeginState';
import type { ToplevelDragEnded } from './bindings/ToplevelDragEnded';
import type { ToplevelDragStarted } from './bindings/ToplevelDragStarted';
import type { ToplevelOutcome } from './bindings/ToplevelOutcome';
import type { UnavailableFeature } from './bindings/UnavailableFeature';

const PREFIX = 'plugin:window-tearoff|';

/** Sent to the ghost window with the drag's payload. */
export const PAYLOAD_EVENT = 'window-tearoff://payload';
/** Sent to the window that began a drag when it ran too long and was ended. */
export const TIMEOUT_EVENT = 'window-tearoff://timeout';
/** Sent to the window that began a drag when the cursor value froze (`true`) or moved again (`false`). */
export const CURSOR_STALE_EVENT = 'window-tearoff://cursor-stale';

/** Sent to the window that began a toplevel drag once the compositor has taken it: it gets no pointer events until the drag ends. */
export const TOPLEVEL_DRAG_STARTED_EVENT = 'window-tearoff://toplevel-drag-started';
/** Sent, when a toplevel drag ends however it ends, to the window that began it and to the window that was dragged. */
export const TOPLEVEL_DRAG_ENDED_EVENT = 'window-tearoff://toplevel-drag-ended';
/** Sent to the window a toplevel drag's payload was dropped on. */
export const PAYLOAD_DROPPED_EVENT = 'window-tearoff://tab-dropped';

/** The features `getStatus` can report. */
export type Feature = 'ghost' | 'cursor_follow' | 'window_position' | 'hit_test' | 'toplevel_drag';

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

/**
 * Drags the window labelled `windowLabel` with the pointer through the compositor (feature `toplevel_drag`), from the press in the calling window.
 * The window follows the pointer even outside every window, stays where it is dropped and snaps back on cancel.
 * It may be the caller itself, or another window created hidden for the drag.
 * `payload` is opaque JSON that a window it is dropped on receives (`onPayloadDropped`); `grabOffset` is where the pointer holds the window, in logical pixels from its top-left.
 * Resolves to `{ state: 'unavailable' }` where the system cannot do it, and nothing starts.
 * Once the compositor has taken the drag the caller gets `onToplevelDragStarted`, then no pointer events until it ends; `onToplevelDragEnded` says how.
 */
export function beginToplevelDrag(
	payload: unknown,
	windowLabel: string,
	grabOffset: Point,
): Promise<ToplevelBeginReport> {
	return cmd<ToplevelBeginReport>('begin_toplevel_drag', { payload, windowLabel, grabOffset });
}

/** Cancels the toplevel drag in progress, if any; it ends as `cancelled`. */
export function endToplevelDrag(): Promise<void> {
	return cmd<void>('end_toplevel_drag');
}

/** How the toplevel drag that moved the calling window ended, once, for a page that was still loading when it did. */
export function takeToplevelDragResult(): Promise<ToplevelDragEnded | null> {
	return cmd<ToplevelDragEnded | null>('take_toplevel_drag_result');
}

/** Listens, in the window that began a toplevel drag, for the compositor taking it. */
export function onToplevelDragStarted(
	handler: (_started: ToplevelDragStarted) => void,
): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<ToplevelDragStarted>(
		TOPLEVEL_DRAG_STARTED_EVENT,
		(event) => handler(event.payload),
	);
}

/** Listens, in the window that began a toplevel drag or the one dragged, for its end. */
export function onToplevelDragEnded(
	handler: (_ended: ToplevelDragEnded) => void,
): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<ToplevelDragEnded>(TOPLEVEL_DRAG_ENDED_EVENT, (event) =>
		handler(event.payload),
	);
}

/** Listens, in any window, for a toplevel drag's payload being dropped on it. */
export function onPayloadDropped(handler: (_dropped: PayloadDropped) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<PayloadDropped>(PAYLOAD_DROPPED_EVENT, (event) =>
		handler(event.payload),
	);
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
	PayloadDropped,
	PluginStatus,
	Point,
	Region,
	Size,
	ToplevelBeginReport,
	ToplevelBeginState,
	ToplevelDragEnded,
	ToplevelDragStarted,
	ToplevelOutcome,
	UnavailableFeature,
};
