// Free space on the volume of the folder being shown, refreshed as the folder changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VolumeSpace } from '@liminal-hq/waypoint-protocol/generated/VolumeSpace';
import { useEffect, useState } from 'react';
import type { VfsClient } from '../services/vfsClient';

/** Changes in a folder can change free space; re-read it at most this often while they keep coming. */
export const FREE_SPACE_REFRESH_MS = 2000;

/**
 * The space on `location`'s volume, or `null` while unknown, when Rust cannot tell, and for no
 * location. It is read when the location changes and again (throttled) when the listing's
 * `revision` moves, since creating or deleting files changes it.
 */
export function useFreeSpace(
	client: VfsClient,
	location: Location | undefined,
	revision: number,
): VolumeSpace | null {
	const [state, setState] = useState<{ uri: string; space: VolumeSpace | null } | null>(null);
	const uri = location?.uri;

	useEffect(() => {
		if (!location) return;
		let cancelled = false;
		// The first read is immediate; a revision change after it waits, so a burst of live patches
		// asks once.
		const first = state?.uri !== location.uri;
		const timer = setTimeout(
			() => {
				client.getFreeSpace(location).then(
					(space) => {
						if (!cancelled) setState({ uri: location.uri, space });
					},
					() => {
						if (!cancelled) setState({ uri: location.uri, space: null });
					},
				);
			},
			first ? 0 : FREE_SPACE_REFRESH_MS,
		);
		return () => {
			cancelled = true;
			clearTimeout(timer);
		};
		// Reading `state` here only decides how long to wait; it must not retrigger the effect.
	}, [client, uri, revision]);

	return location && state?.uri === location.uri ? state.space : null;
}
