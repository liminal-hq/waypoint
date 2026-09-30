// Reads a location's parent and breadcrumbs from Rust once per location and shares the answer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { LocationInfo } from '@liminal-hq/waypoint-protocol/generated/LocationInfo';
import { useEffect, useState } from 'react';
import type { VfsClient } from '../services/vfsClient';

const MAX_CACHED = 256;
const caches = new WeakMap<VfsClient, Map<string, Promise<LocationInfo>>>();

/**
 * The parent and breadcrumb segments of `location`. Every tab title, the path bar and the Up
 * button ask about the same few locations again and again, so one answer per `uri` is kept per
 * client; a failed read is forgotten so the next ask retries.
 */
export function describeLocation(client: VfsClient, location: Location): Promise<LocationInfo> {
	let cache = caches.get(client);
	if (!cache) {
		cache = new Map();
		caches.set(client, cache);
	}
	const known = cache.get(location.uri);
	if (known) return known;
	if (cache.size >= MAX_CACHED) cache.clear();
	const fresh = client.describeLocation(location);
	cache.set(location.uri, fresh);
	fresh.catch(() => cache.delete(location.uri));
	return fresh;
}

/**
 * `location`'s info once Rust has answered, `null` until then and for no location. While the next
 * location's answer is on its way the previous one is returned, so the path bar does not flash.
 */
export function useLocationInfo(
	client: VfsClient,
	location: Location | undefined,
): LocationInfo | null {
	const [loaded, setLoaded] = useState<{ uri: string; info: LocationInfo } | null>(null);
	const uri = location?.uri;

	useEffect(() => {
		if (!location) return;
		let cancelled = false;
		describeLocation(client, location).then(
			(info) => {
				if (!cancelled) setLoaded({ uri: location.uri, info });
			},
			() => {},
		);
		return () => {
			cancelled = true;
		};
		// A new object for the same folder must not ask again.
	}, [client, uri]);

	return location ? (loaded?.info ?? null) : null;
}
