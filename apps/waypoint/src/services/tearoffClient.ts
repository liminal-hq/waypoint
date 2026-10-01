// The tear-off plugin as the tab drag uses it: feature flags, the ghost, drop regions and the drop report
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	DragHover,
	DragLeave,
	DropReport,
	Hit,
	PayloadDropped,
	Point,
	Region,
	Size,
	ToplevelBeginReport,
	ToplevelDragEnded,
	ToplevelDragStarted,
} from '@liminal-hq/plugin-window-tearoff';
import type { Unsubscribe } from './vfsClient';

export type {
	DragHover,
	DragLeave,
	DropReport,
	Hit,
	PayloadDropped,
	Point,
	Region,
	Size,
	ToplevelBeginReport,
	ToplevelDragEnded,
	ToplevelDragStarted,
};

/**
 * What the plugin can do on this system. Nothing here is a platform check: each one is a probe
 * the plugin made, so a system that cannot do it (Wayland) simply reports `false` and the drag
 * takes the in-page path (D94).
 */
export interface TearoffFeatures {
	/** A ghost window can be shown and follow the pointer outside the window. */
	ghost: boolean;
	/** The native cursor is live while a button is held. */
	cursorFollow: boolean;
	/** A new window can be placed at a position, so the cursor can place it. */
	windowPosition: boolean;
	/** Another window's drop regions can be hit-tested. */
	hitTest: boolean;
	/** The compositor can move a real window with the pointer for the whole drag (Wayland, `xdg-toplevel-drag`). */
	toplevelDrag: boolean;
}

/** Every feature off: what a window has before the plugin has answered, and without the plugin. */
export const NO_TEAROFF: TearoffFeatures = {
	ghost: false,
	cursorFollow: false,
	windowPosition: false,
	hitTest: false,
	toplevelDrag: false,
};

/** What the drag draws on the ghost card; the plugin carries it as opaque JSON. */
export interface GhostPayload {
	title?: string;
	count?: number;
	label?: string;
}

export type BeginResult = 'following' | 'noGhost' | 'alreadyActive';

/** The Tauri events one window sends another so it can show where a tab dragged from the first would land. */
export const MERGE_HOVER_EVENT = 'waypoint://merge-hover';
export const MERGE_LEAVE_EVENT = 'waypoint://merge-leave';

/**
 * Where a drag from another window is over this one, as the window it is over hears of it. `x` and
 * `y` are the pointer in logical pixels from the top-left of the page; `region` is the drop region
 * (`dropRegions.ts`) the plugin found under it, which is what a release merges at, and null for a
 * pointer over the window but over no region.
 */
export interface MergeHover {
	x: number;
	y: number;
	region: string | null;
	/** How many tabs the drag carries. */
	count: number;
	/** Whether they are pinned where they come from, which decides which side of the pinned boundary they land on. */
	pinned: boolean;
}

/** Everything the new-window phase of a tab drag asks of the tear-off plugin. */
export interface TearoffClient {
	/** The probed features. The first call can take a second on Wayland, so it is asked once and kept. */
	features(): Promise<TearoffFeatures>;
	begin(payload: GhostPayload, grabOffset: Point, size: Size): Promise<BeginResult>;
	update(payload: GhostPayload): Promise<void>;
	end(outcome: 'drop' | 'cancel'): Promise<DropReport>;
	/** The registered region under the cursor now, without ending the drag. */
	hitTest(): Promise<Hit | null>;
	/** Replaces this window's drop regions, in logical pixels from the top-left of its content. */
	setDropRegions(regions: Region[]): Promise<void>;
	/** The plugin ended a drag that ran too long. */
	onTimeout(listener: () => void): Unsubscribe;
	/** The cursor froze while a button was held (`true`) or moved again (`false`). */
	onCursorStale(listener: (stale: boolean) => void): Unsubscribe;

	/** Tells another window a ghost drag is over its strip (`merge-hover`); the caller sends it on a change of slot, not on every poll. */
	sendMergeHover(window: string, hover: MergeHover): Promise<void>;
	/** Tells another window the drag it was told of has left it, or ended. */
	sendMergeLeave(window: string): Promise<void>;
	/** A ghost drag from another window is over this one. */
	onMergeHover(listener: (hover: MergeHover) => void): Unsubscribe;
	onMergeLeave(listener: () => void): Unsubscribe;
	/** A toplevel drag's payload is over this window (the plugin's own events; the compositor's drag gives the page no pointer events). */
	onDragHover(listener: (hover: DragHover) => void): Unsubscribe;
	onDragLeave(listener: (leave: DragLeave) => void): Unsubscribe;

	/** Keeps the next window the session makes hidden (`true`), or stops doing so; the toplevel drag shows it. */
	holdNextWindow(on: boolean): Promise<void>;
	/** Shows a main window that was made hidden for a drag that then could not start, so what it holds is not lost. */
	showWindow(label: string): Promise<void>;
	/** Drags the window `windowLabel` with the compositor, from the press in this window. */
	beginToplevelDrag(
		payload: unknown,
		windowLabel: string,
		grabOffset: Point,
	): Promise<ToplevelBeginReport>;
	/** Cancels the toplevel drag in progress. */
	endToplevelDrag(): Promise<void>;
	/** How the drag that moved this window ended, for a page that loaded after it did; read once. */
	takeToplevelResult(): Promise<ToplevelDragEnded | null>;
	/** The compositor took the drag this window began: the page gets no pointer events until it ends. */
	onToplevelStarted(listener: (started: ToplevelDragStarted) => void): Unsubscribe;
	/** A toplevel drag this window began, or that moved this window, ended. */
	onToplevelEnded(listener: (ended: ToplevelDragEnded) => void): Unsubscribe;
	/** A toplevel drag's payload was dropped on this window. */
	onPayloadDropped(listener: (dropped: PayloadDropped) => void): Unsubscribe;
}
