// The real TrashClient: the Trash's state from the file system plugin and its jobs from the operations plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getTrashInfo } from '@liminal-hq/waypoint-plugin-vfs';
import * as ops from '@liminal-hq/waypoint-plugin-ops';
import type { Unsubscribe } from '../services/vfsClient';
import type { TrashClient } from './trashClient';

/** A `TrashClient` over the `waypoint-vfs` and `waypoint-ops` plugins. Create one per window. */
export function createTauriTrashClient(): TrashClient {
	return {
		getInfo: (withBytes) => getTrashInfo(withBytes),
		submit: (request) => ops.submit(request),
		onEvent(listener): Unsubscribe {
			let stopped = false;
			let unlisten: (() => void) | undefined;
			ops
				.onOpsEvent((event) => {
					if (!stopped) listener(event);
				})
				.then(
					(stop) => {
						// Unsubscribed before the listener was registered: undo it straight away.
						if (stopped) stop();
						else unlisten = stop;
					},
					(error) => console.error('could not listen for operations events', error),
				);
			return () => {
				stopped = true;
				unlisten?.();
			};
		},
	};
}
