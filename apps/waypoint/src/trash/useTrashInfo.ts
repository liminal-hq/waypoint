// Keeps the Trash place's state current: read at start, on focus, after every job, and on a slow timer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TrashInfo } from '@liminal-hq/waypoint-protocol/generated/TrashInfo';
import { useEffect, useState } from 'react';
import type { TrashClient } from './trashClient';

/** How often the count is read while the window is visible. Reading it lists the Trash. */
export const TRASH_INFO_INTERVAL_MS = 10_000;

/**
 * The Trash's state, or `null` until the first read (and without a client). The count is read
 * again when the window gains focus, whenever a job finishes (a restore, a delete, an empty, or a
 * move to the Trash), and every ten seconds while the window is visible, which is how another
 * program's change shows. A read that fails leaves the last answer in place.
 */
export function useTrashInfo(client: TrashClient | null): TrashInfo | null {
	const [info, setInfo] = useState<TrashInfo | null>(null);
	useEffect(() => {
		if (!client) return;
		let live = true;
		// Counts the reads begun, so a slow one that finishes after a newer one is dropped.
		let latest = 0;
		const read = () => {
			const mine = ++latest;
			client.getInfo().then(
				(next) => {
					if (live && mine === latest) setInfo((have) => (same(have, next) ? have : next));
				},
				(error) => console.warn('could not read the Trash', error),
			);
		};
		read();
		const onFocus = () => read();
		window.addEventListener('focus', onFocus);
		const timer = window.setInterval(() => {
			if (document.visibilityState !== 'hidden') read();
		}, TRASH_INFO_INTERVAL_MS);
		const stop = client.onEvent((event) => {
			if (event.kind === 'jobChanged' && event.job.state.state === 'done') read();
		});
		return () => {
			live = false;
			window.removeEventListener('focus', onFocus);
			window.clearInterval(timer);
			stop();
		};
	}, [client]);
	return client ? info : null;
}

function same(a: TrashInfo | null, b: TrashInfo): boolean {
	return a !== null && a.available === b.available && a.reason === b.reason && a.count === b.count;
}
