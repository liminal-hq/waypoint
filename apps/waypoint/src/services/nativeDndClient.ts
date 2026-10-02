// The native drag and drop plugin as the file drag uses it: what works here, an outbound drag, and the events of drags from outside the window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	DragAction,
	DragEnded,
	DragOutcome,
	DropEvent,
	EnterEvent,
	ErrorKind,
	LeaveEvent,
	Modifiers,
	OverEvent,
	PluginStatus,
	Position,
} from '@liminal-hq/plugin-native-dnd';
import type { Unsubscribe } from './vfsClient';

export type {
	DragAction,
	DragEnded,
	DragOutcome,
	DropEvent,
	EnterEvent,
	ErrorKind,
	LeaveEvent,
	Modifiers,
	OverEvent,
	Position,
};

/** The features the file drag asks about. */
export type NativeDndFeature = 'inbound' | 'outbound' | 'modifiers' | 'clipboard';

/**
 * What works on this system, in the form the file drag decides by. A feature that does not work is
 * hidden or skipped, never half-offered, and `reasons` says why for the Services panel and the log.
 */
export interface NativeDndAvailability {
	/** Files dragged in from other applications and windows reach the page. */
	inbound: boolean;
	/** A drag that leaves the window continues as a system drag. */
	outbound: boolean;
	/** The modifier keys are known while files are dragged in; on Wayland they are not. */
	modifiers: boolean;
	/** The system file clipboard works. */
	clipboard: boolean;
	reasons: Partial<Record<NativeDndFeature, string>>;
	/** `wayland`, `x11`, `windows` or `none`. */
	displayServer: string;
}

/** Nothing works: the plugin is missing, or says so. */
export const NO_NATIVE_DND: NativeDndAvailability = {
	inbound: false,
	outbound: false,
	modifiers: false,
	clipboard: false,
	reasons: {},
	displayServer: 'none',
};

const FEATURES: NativeDndFeature[] = ['inbound', 'outbound', 'modifiers', 'clipboard'];

/** The plugin's `get_status` as the file drag reads it. */
export function availabilityOf(status: PluginStatus): NativeDndAvailability {
	const reasons: Partial<Record<NativeDndFeature, string>> = {};
	for (const feature of FEATURES) {
		const report = status.features[feature];
		if (!report.available && report.reason) reasons[feature] = report.reason;
	}
	return {
		inbound: status.features.inbound.available,
		outbound: status.features.outbound.available,
		modifiers: status.features.modifiers.available,
		clipboard: status.features.clipboard.available,
		reasons,
		displayServer: status.displayServer,
	};
}

/** What an outbound drag offers. */
export interface OutboundRequest {
	/** The lossless `file://` URIs. */
	uris: string[];
	/** What the target may do; at least one. */
	actions: DragAction[];
}

/** An outbound drag that has started. `ended` is set where the command only returns when the drag has finished (Windows). */
export interface OutboundStarted {
	id: number;
	ended: DragEnded | null;
}

/** Why a command failed, as the plugin's error kinds say; `null` for a failure that is not the plugin's. */
export function nativeDndErrorKind(error: unknown): ErrorKind | null {
	const kind = (error as { kind?: unknown } | null)?.kind;
	return typeof kind === 'string' ? (kind as ErrorKind) : null;
}

/**
 * Waypoint's side of the `native-dnd` plugin's drag and drop. Events are delivered to this window
 * only, and carry the label of the window they are for. `FakeNativeDndClient` serves the same
 * contract from a script.
 */
export interface NativeDndClient {
	/** What works here. Asked once: it does not change while the app runs. */
	status(): Promise<NativeDndAvailability>;
	/**
	 * Starts a system drag of files from this window, which must be in the middle of a press of the
	 * primary button. Rejects with `{ kind, message }`: `buttonNotPressed`, `alreadyActive`,
	 * `invalid`, `unsupported` or `failed`.
	 */
	startDrag(request: OutboundRequest): Promise<OutboundStarted>;
	/** Files first move over this window. */
	onEnter(listener: (event: EnterEvent) => void): Unsubscribe;
	/** Files move over this window. */
	onOver(listener: (event: OverEvent) => void): Unsubscribe;
	/** Files are dropped on this window. */
	onDrop(listener: (event: DropEvent) => void): Unsubscribe;
	/** Files leave this window without a drop. */
	onLeave(listener: (event: LeaveEvent) => void): Unsubscribe;
	/** An outbound drag this window started has ended, however it ended. */
	onDragEnded(listener: (event: DragEnded) => void): Unsubscribe;
}
