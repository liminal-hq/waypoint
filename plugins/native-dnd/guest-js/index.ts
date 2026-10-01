// Exposes typed guest-side wrappers for the native-dnd plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { ClipboardFiles } from './bindings/ClipboardFiles';
import type { DisplayServer } from './bindings/DisplayServer';
import type { DragAction } from './bindings/DragAction';
import type { DragEnded } from './bindings/DragEnded';
import type { DragIcon } from './bindings/DragIcon';
import type { DragOutcome } from './bindings/DragOutcome';
import type { DropEvent } from './bindings/DropEvent';
import type { EnterEvent } from './bindings/EnterEvent';
import type { ErrorKind } from './bindings/ErrorKind';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { Features } from './bindings/Features';
import type { LeaveEvent } from './bindings/LeaveEvent';
import type { Modifiers } from './bindings/Modifiers';
import type { NativeDndError } from './bindings/NativeDndError';
import type { OverEvent } from './bindings/OverEvent';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Position } from './bindings/Position';
import type { StartDragReport } from './bindings/StartDragReport';
import type { StartDragRequest } from './bindings/StartDragRequest';

const PREFIX = 'plugin:native-dnd|';

/** Sent to a window when files first move over it. */
export const ENTER_EVENT = 'native-dnd://enter';
/** Sent to a window as files move over it. */
export const OVER_EVENT = 'native-dnd://over';
/** Sent to a window when files are dropped on it. */
export const DROP_EVENT = 'native-dnd://drop';
/** Sent to a window when files leave it without being dropped. */
export const LEAVE_EVENT = 'native-dnd://leave';
/** Sent to the window that started an outbound drag when it ends however it ends. */
export const DRAG_ENDED_EVENT = 'native-dnd://drag-ended';
/** Sent to every window when the file clipboard may have changed. */
export const CLIPBOARD_CHANGED_EVENT = 'native-dnd://clipboard-changed';

/** The features `getStatus` reports. */
export type Feature = keyof Features;

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/**
 * Reports which native drag and drop features work on this system, and why the others do not.
 * Where nothing works every feature says why and the commands reject with `unsupported`.
 */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** True if `feature` works according to `status`. */
export function hasFeature(status: PluginStatus, feature: Feature): boolean {
	return status.features[feature].available;
}

/**
 * Starts an outbound drag of `request.uris` (`file://` URIs) from the calling window, which must be in the middle of a press of the primary mouse button.
 * Rejects with `buttonNotPressed` if it is not, `alreadyActive` while another drag runs, `invalid` for a malformed request and `unsupported` where outbound drags do not work.
 * On Linux it resolves as soon as the drag has started (`ended` is null) and `onDragEnded` reports the end; on Windows it resolves when the drag has finished, with `ended` set (`onDragEnded` still fires).
 * The page gets no pointer events from the moment the drag starts, so reset its own pointer state in `onDragEnded`.
 */
export function startDrag(request: StartDragRequest): Promise<StartDragReport> {
	return cmd<StartDragReport>('start_drag', { request });
}

/**
 * Puts `files` on the system clipboard, to copy or (with `cut`) to move.
 * On Wayland the compositor accepts it only shortly after a key press or click in the app, so call it from the handler of that action.
 */
export function setFiles(files: ClipboardFiles): Promise<void> {
	return cmd<void>('set_files', { files });
}

/** The files on the system clipboard, or null when it holds none. */
export function getFiles(): Promise<ClipboardFiles | null> {
	return cmd<ClipboardFiles | null>('get_files');
}

/** True if `error` is the value a plugin command rejects with, optionally of the given `kind`. */
export function isNativeDndError(error: unknown, kind?: ErrorKind): error is NativeDndError {
	if (typeof error !== 'object' || error === null) return false;
	const candidate = error as Partial<NativeDndError>;
	return (
		typeof candidate.kind === 'string' &&
		typeof candidate.message === 'string' &&
		(kind === undefined || candidate.kind === kind)
	);
}

/** Listens, in the calling window, for files first moving over it. Only windows whose drag-drop handler is on get these. */
export function onEnter(handler: (_event: EnterEvent) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<EnterEvent>(ENTER_EVENT, (event) =>
		handler(event.payload),
	);
}

/** Listens, in the calling window, for files moving over it (every pointer move, so keep the handler cheap). */
export function onOver(handler: (_event: OverEvent) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<OverEvent>(OVER_EVENT, (event) => handler(event.payload));
}

/** Listens, in the calling window, for a drop. `selfDrop` marks the end of a drag this app started. */
export function onDrop(handler: (_event: DropEvent) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<DropEvent>(DROP_EVENT, (event) => handler(event.payload));
}

/** Listens, in the calling window, for files leaving it without a drop. */
export function onLeave(handler: (_event: LeaveEvent) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<LeaveEvent>(LEAVE_EVENT, (event) =>
		handler(event.payload),
	);
}

/** Listens, in the window that started an outbound drag, for its end. */
export function onDragEnded(handler: (_event: DragEnded) => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen<DragEnded>(DRAG_ENDED_EVENT, (event) =>
		handler(event.payload),
	);
}

/** Listens for the clipboard's owner changing, which may mean `getFiles` now answers differently. */
export function onClipboardChanged(handler: () => void): Promise<UnlistenFn> {
	return getCurrentWebviewWindow().listen(CLIPBOARD_CHANGED_EVENT, () => handler());
}

export type {
	ClipboardFiles,
	DisplayServer,
	DragAction,
	DragEnded,
	DragIcon,
	DragOutcome,
	DropEvent,
	EnterEvent,
	ErrorKind,
	FeatureStatus,
	Features,
	LeaveEvent,
	Modifiers,
	NativeDndError,
	OverEvent,
	PluginStatus,
	Position,
	StartDragReport,
	StartDragRequest,
};
