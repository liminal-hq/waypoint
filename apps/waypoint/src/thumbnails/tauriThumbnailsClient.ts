// The real ThumbnailsClient: the `src-tauri` bridge commands, and the plugin's own status
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getStatus } from '@liminal-hq/plugin-thumbnails';
import { Channel, invoke } from '@tauri-apps/api/core';
import type {
	EntryRequest,
	LocationRequest,
	ThumbEvent,
	ThumbnailsClient,
	ThumbSize,
	Ticket,
} from './thumbnailsClient';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';

function channelFor(onEvent: (event: ThumbEvent) => void): Channel<ThumbEvent> {
	const channel = new Channel<ThumbEvent>();
	channel.onmessage = onEvent;
	return channel;
}

/** A `ThumbnailsClient` over the bridge commands in `src-tauri`. Create one per window. */
export function createTauriThumbnailsClient(): ThumbnailsClient {
	return {
		getStatus: () => getStatus(),
		requestEntries: (handle: ListingHandle, items: EntryRequest[], size: ThumbSize, onEvent) =>
			invoke<Ticket>('thumbnails_request_entries', {
				handle,
				items,
				size,
				onEvent: channelFor(onEvent),
			}),
		requestLocations: (items: LocationRequest[], size: ThumbSize, onEvent) =>
			invoke<Ticket>('thumbnails_request_locations', {
				items,
				size,
				onEvent: channelFor(onEvent),
			}),
		prioritise: (ticket, keys) => invoke<void>('thumbnails_prioritise', { ticket, keys }),
		cancel: (ticket) => invoke<boolean>('thumbnails_cancel', { ticket }),
	};
}
