// The real NativeDndClient: the native-dnd plugin's drag, drop and status commands and events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as dnd from '@liminal-hq/plugin-native-dnd';
import {
	availabilityOf,
	NO_NATIVE_DND,
	type NativeDndAvailability,
	type NativeDndClient,
} from './nativeDndClient';
import type { Unsubscribe } from './vfsClient';

/** Turns a plugin's promise of a listener into a function that stops listening, even if called before it has arrived. */
function listen(register: () => Promise<() => void>, what: string): Unsubscribe {
	let stopped = false;
	let unlisten: (() => void) | undefined;
	register().then(
		(stop) => {
			if (stopped) stop();
			else unlisten = stop;
		},
		(error: unknown) => console.warn(`could not listen for ${what}`, error),
	);
	return () => {
		stopped = true;
		unlisten?.();
		unlisten = undefined;
	};
}

/** A `NativeDndClient` over the plugin. A plugin that cannot answer is reported as offering nothing. */
export function createTauriNativeDndClient(): NativeDndClient {
	let availability: Promise<NativeDndAvailability> | null = null;
	return {
		status() {
			availability ??= dnd.getStatus().then(availabilityOf, (error: unknown) => {
				console.warn('could not read the native drag and drop status', error);
				return NO_NATIVE_DND;
			});
			return availability;
		},
		startDrag: (request) => dnd.startDrag({ ...request, icon: null }),
		onEnter: (listener) => listen(() => dnd.onEnter(listener), 'files entering the window'),
		onOver: (listener) => listen(() => dnd.onOver(listener), 'files moving over the window'),
		onDrop: (listener) => listen(() => dnd.onDrop(listener), 'files dropped on the window'),
		onLeave: (listener) => listen(() => dnd.onLeave(listener), 'files leaving the window'),
		onDragEnded: (listener) => listen(() => dnd.onDragEnded(listener), 'the end of a drag'),
	};
}
