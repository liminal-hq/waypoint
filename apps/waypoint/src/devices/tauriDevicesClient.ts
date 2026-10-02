// The real DevicesClient: the volumes plugin's commands and its change event
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as volumes from '@liminal-hq/plugin-volumes';
import type { Unsubscribe } from '../services/vfsClient';
import type { DevicesClient } from './devicesClient';

/** A `DevicesClient` over the `volumes` plugin. Create one per window. */
export function createTauriDevicesClient(): DevicesClient {
	return {
		getStatus: () => volumes.getStatus(),
		list: () => volumes.list(),
		refreshSpace: (id) => volumes.refreshSpace(id),
		mount: (id) => volumes.mount(id),
		unmount: (id) => volumes.unmount(id),
		eject: (id) => volumes.eject(id),
		unlock: (id, passphrase) => volumes.unlock(id, passphrase),
		onChanged(listener): Unsubscribe {
			let stopped = false;
			let unlisten: (() => void) | undefined;
			volumes
				.onChanged((event) => {
					if (!stopped) listener(event);
				})
				.then(
					(stop) => {
						// Unsubscribed before the listener was registered: undo it straight away.
						if (stopped) stop();
						else unlisten = stop;
					},
					(error) => console.error('could not listen for volume changes', error),
				);
			return () => {
				stopped = true;
				unlisten?.();
			};
		},
	};
}
