// The real OsClipboardClient: the native-dnd plugin's file clipboard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as dnd from '@liminal-hq/plugin-native-dnd';
import type { OsClipboardClient } from './osClipboardClient';
import type { Unsubscribe } from './vfsClient';

/** An `OsClipboardClient` over the plugin. Availability is read once: it does not change while the app runs. */
export function createTauriOsClipboardClient(): OsClipboardClient {
	let availability: Promise<boolean> | null = null;
	return {
		available() {
			availability ??= dnd.getStatus().then(
				(status) => dnd.hasFeature(status, 'clipboard'),
				() => false,
			);
			return availability;
		},
		setFiles: (files) => dnd.setFiles(files),
		getFiles: () => dnd.getFiles(),
		onChange(listener): Unsubscribe {
			let stopped = false;
			let unlisten: (() => void) | undefined;
			dnd.onClipboardChanged(listener).then(
				(stop) => {
					if (stopped) stop();
					else unlisten = stop;
				},
				(error: unknown) => console.warn('could not listen for the file clipboard', error),
			);
			return () => {
				stopped = true;
				unlisten?.();
				unlisten = undefined;
			};
		},
	};
}
