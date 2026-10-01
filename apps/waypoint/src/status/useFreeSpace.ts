// Free space on the volume of the folder being shown, refreshed as the folder changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VolumeSpace } from '@liminal-hq/waypoint-protocol/generated/VolumeSpace';
import { useEffect, useRef, useState } from 'react';
import type { VfsClient } from '../services/vfsClient';

/** Changes in a folder can change free space; re-read it at most this often while they keep coming (a throttle, so a long burst still refreshes). */
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
	const latest = useRef({ location, revision });
	latest.current = { location, revision };
	const read = useRef<(() => void) | null>(null);
	const lastReadAt = useRef(0);
	const pending = useRef<ReturnType<typeof setTimeout> | null>(null);
	const seenRevision = useRef(revision);

	// A new location is read at once.
	useEffect(() => {
		const target = latest.current.location;
		if (!target) return;
		let cancelled = false;
		seenRevision.current = latest.current.revision;
		const run = () => {
			lastReadAt.current = Date.now();
			client.getFreeSpace(target).then(
				(space) => {
					if (!cancelled) setState({ uri: target.uri, space });
				},
				() => {
					if (!cancelled) setState({ uri: target.uri, space: null });
				},
			);
		};
		read.current = run;
		run();
		return () => {
			cancelled = true;
			read.current = null;
			if (pending.current) clearTimeout(pending.current);
			pending.current = null;
		};
	}, [client, uri]);

	// A revision change reads at once if the last read was long enough ago (leading edge), and
	// otherwise once when the interval is up (trailing edge), however many more changes arrive.
	useEffect(() => {
		if (seenRevision.current === revision) return;
		seenRevision.current = revision;
		if (!read.current || pending.current) return;
		const wait = FREE_SPACE_REFRESH_MS - (Date.now() - lastReadAt.current);
		if (wait <= 0) {
			read.current();
			return;
		}
		pending.current = setTimeout(() => {
			pending.current = null;
			read.current?.();
		}, wait);
	}, [revision]);

	return location && state?.uri === location.uri ? state.space : null;
}
