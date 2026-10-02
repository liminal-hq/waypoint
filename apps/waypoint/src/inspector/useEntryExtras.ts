// The facts about an entry that cost a round trip each: where it is, the space around it and the application that opens it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VolumeSpace } from '@liminal-hq/waypoint-protocol/generated/VolumeSpace';
import { useEffect, useState } from 'react';
import { useOptionalVfsClient } from '../browse/VfsClientContext';
import { openWithAbilities, useOpenWithService } from '../openWith/OpenWithContext';
import { isLocalUri } from '../openWith/openWithUris';

/** What an entry's own location leads to. */
export interface EntryExtras {
	/** The entry's own location; `null` until it is known. */
	location: Location | null;
	/** Space on its volume, read for a folder only; `null` when unknown. */
	space: VolumeSpace | null;
	/** The default application's name: a string, `null` for none, `undefined` while unknown or not asked. */
	defaultApp: string | null | undefined;
}

/**
 * Reads, once `enabled` (the selection has settled and the Properties tab is showing), the
 * entry's location, then for a folder the free space of its volume and for a local file the
 * default application the mime-apps plugin names. Each answer is for the entry it was asked about
 * and dropped if the subject has moved on; a failure leaves that row out.
 */
export function useEntryExtras(
	handle: ListingHandle | null,
	id: EntryId | null,
	folder: boolean,
	enabled: boolean,
): EntryExtras {
	const vfs = useOptionalVfsClient();
	const openWith = useOpenWithService();
	const canAsk = openWith !== null && openWithAbilities(openWith.status).list;
	const key = handle !== null && id !== null ? `${handle}:${id}` : null;
	const [result, setResult] = useState<{ key: string; extras: EntryExtras } | null>(null);

	useEffect(() => {
		if (!enabled || !vfs || handle === null || id === null || key === null) return;
		let live = true;
		const publish = (patch: Partial<EntryExtras>) => {
			if (!live) return;
			setResult((current) => {
				const base: EntryExtras =
					current?.key === key
						? current.extras
						: { location: null, space: null, defaultApp: undefined };
				return { key, extras: { ...base, ...patch } };
			});
		};
		(async () => {
			const location = await vfs.entryLocation(handle, id);
			publish({ location });
			if (folder) {
				const space = await vfs.getFreeSpace(location).catch(() => null);
				publish({ space });
			} else if (canAsk && openWith && isLocalUri(location.uri)) {
				const handlers = await openWith.client.handlers([location.uri]).catch(() => null);
				if (handlers) publish({ defaultApp: handlers.default?.name ?? null });
			}
		})().catch((error: unknown) => console.warn('could not read where the entry is', error));
		return () => {
			live = false;
		};
	}, [enabled, vfs, handle, id, key, folder, canAsk, openWith]);

	if (key === null || result?.key !== key) {
		return { location: null, space: null, defaultApp: undefined };
	}
	return result.extras;
}
