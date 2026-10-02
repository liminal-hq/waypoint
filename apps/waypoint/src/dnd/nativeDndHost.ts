// Connects the native drag and drop plugin's events to the file drag: files dragged in feed the same targets, and the end of an outbound drag is heard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	DropEvent,
	EnterEvent,
	NativeDndAvailability,
	NativeDndClient,
	Modifiers,
} from '../services/nativeDndClient';
import { NO_NATIVE_DND } from '../services/nativeDndClient';
import { NO_MODIFIERS, type DropModifiers } from './dropAction';
import type { FileDrag, NativeFeed } from './fileDrag';
import { droppedFiles } from './nativeDropModel';

/**
 * Set on the root element while files from outside this window are held over it, so a window that
 * is itself the drop target (the Shelf window) can show it with a cue around the whole window.
 */
export const WINDOW_DROP_ATTRIBUTE = 'data-window-drop';

const rootElement = (): HTMLElement | null =>
	typeof document === 'undefined' ? null : document.documentElement;

export interface NativeDndHostDeps {
	client: NativeDndClient;
	drag: FileDrag;
	/** The window's label; events for another window are not this one's. `null` before the window's queue is known. */
	windowLabel(): string | null;
	/** What works here, once the plugin has said. */
	onAvailability?(availability: NativeDndAvailability): void;
}

/**
 * Listens to the plugin for this window and drives the file drag:
 * - `enter` begins a drag of the files (the `file:` URIs only; text, links and other kinds of drag
 *   are ignored), `over` moves it, `leave` ends it without a drop, and `drop` releases it, so the
 *   targets, the pill, the spring-loading and the default action are the ones an in-page drag has;
 * - a drop that arrives with no `enter` before it begins the drag at the drop;
 * - the modifier keys come from the events where the platform reports them and read as released
 *   where it does not (Wayland), so the rule alone decides and the picker is the way to choose;
 * - `drag-ended` tells the drag how an outbound drag that began here ended.
 * Returns what stops listening. Inbound events are listened to only where the plugin says they work.
 */
export function connectNativeDnd(deps: NativeDndHostDeps): () => void {
	const { client, drag } = deps;
	let stopped = false;
	let availability = NO_NATIVE_DND;
	const stops: Array<() => void> = [];
	let feed: NativeFeed | null = null;

	const mine = (window: string) => {
		const label = deps.windowLabel();
		return label === null || window === label;
	};
	const keys = (modifiers: Modifiers): DropModifiers =>
		availability.modifiers
			? { ctrl: modifiers.ctrl, shift: modifiers.shift, alt: modifiers.alt }
			: NO_MODIFIERS;

	const begin = (event: EnterEvent | DropEvent) => {
		const files = droppedFiles(event.uris, event.paths);
		if (files.length === 0) return null;
		return drag.beginNative({ files, point: event.position, modifiers: keys(event.modifiers) });
	};

	void client.status().then((found) => {
		if (stopped) return;
		availability = found;
		deps.onAvailability?.(found);
		if (found.inbound) {
			stops.push(
				client.onEnter((event) => {
					if (!mine(event.window)) return;
					feed?.leave();
					feed = begin(event);
					if (feed) rootElement()?.setAttribute(WINDOW_DROP_ATTRIBUTE, '');
				}),
				client.onOver((event) => {
					if (!mine(event.window)) return;
					feed?.move(event.position, keys(event.modifiers));
				}),
				client.onLeave((event) => {
					if (!mine(event.window)) return;
					feed?.leave();
					feed = null;
					rootElement()?.removeAttribute(WINDOW_DROP_ATTRIBUTE);
				}),
				client.onDrop((event) => {
					if (!mine(event.window)) return;
					rootElement()?.removeAttribute(WINDOW_DROP_ATTRIBUTE);
					// Some platforms drop without having entered: the drag begins where it is released.
					const active = feed ?? begin(event);
					feed = null;
					void active?.drop(event.position, keys(event.modifiers), event.selfDrop);
				}),
			);
		}
		if (found.outbound) stops.push(client.onDragEnded((event) => drag.dragEnded(event)));
	});

	return () => {
		stopped = true;
		feed?.leave();
		feed = null;
		rootElement()?.removeAttribute(WINDOW_DROP_ATTRIBUTE);
		for (const stop of stops.splice(0)) stop();
	};
}
