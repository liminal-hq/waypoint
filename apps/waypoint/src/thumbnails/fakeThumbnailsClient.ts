// An in-memory ThumbnailsClient for tests and the browser demo: a log of what was asked, and results the test sends
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PluginStatus, SkipWhy } from '@liminal-hq/plugin-thumbnails';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type {
	EntryRequest,
	LocationRequest,
	ThumbEvent,
	ThumbnailsClient,
	ThumbSize,
	Ticket,
} from './thumbnailsClient';

/** One batch the page queued. */
export interface FakeBatch {
	ticket: Ticket;
	/** Set for a batch of a listing's entries. */
	handle: ListingHandle | null;
	size: ThumbSize;
	/** The page's keys, in the order sent. */
	keys: string[];
	cancelled: boolean;
	/** Every `prioritise` call for it, in order. */
	prioritised: string[][];
}

export interface FakeThumbnailsClient extends ThumbnailsClient {
	readonly batches: FakeBatch[];
	/** Sends `ready` for `key` to every batch that asked for it and has not been cancelled. */
	ready(key: string, url?: string): void;
	fail(key: string, reason?: string): void;
	skip(key: string, why?: SkipWhy): void;
	/** Makes requests slow: they settle when `release` is called. */
	hold(): { release(): void };
}

/** A status for a system where thumbnails work. */
export function workingStatus(): PluginStatus {
	return {
		available: true,
		reason: null,
		flavour: 'freedesktop',
		features: [
			{ name: 'cache', available: true, reason: null, count: null },
			{ name: 'builtin', available: true, reason: null, count: null },
		],
	};
}

/** A status for a system where nothing works, with `message` as the reason. */
export function brokenStatus(message = 'The thumbnail cache cannot be used.'): PluginStatus {
	return {
		available: false,
		reason: { kind: 'unsupported', message },
		flavour: 'unsupported',
		features: [],
	};
}

/** A client that answers nothing until the test does. */
export function createFakeThumbnailsClient(
	status: PluginStatus = workingStatus(),
): FakeThumbnailsClient {
	const batches: FakeBatch[] = [];
	const sinks = new Map<Ticket, (event: ThumbEvent) => void>();
	let next = 1;
	let held: Array<() => void> | null = null;
	const queue = async (
		batch: Omit<FakeBatch, 'ticket' | 'cancelled' | 'prioritised'>,
		onEvent: (event: ThumbEvent) => void,
	) => {
		const ticket = next++;
		batches.push({ ...batch, ticket, cancelled: false, prioritised: [] });
		sinks.set(ticket, onEvent);
		if (held) await new Promise<void>((resolve) => held?.push(resolve));
		return ticket;
	};
	const send = (key: string, event: ThumbEvent) => {
		for (const batch of batches) {
			if (!batch.cancelled && batch.keys.includes(key)) sinks.get(batch.ticket)?.(event);
		}
	};
	return {
		batches,
		getStatus: async () => status,
		requestEntries: (handle, items: EntryRequest[], size, onEvent) =>
			queue({ handle, size, keys: items.map((item) => item.key) }, onEvent),
		requestLocations: (items: LocationRequest[], size, onEvent) =>
			queue({ handle: null, size, keys: items.map((item) => item.key) }, onEvent),
		prioritise: async (ticket, keys) => {
			batches.find((batch) => batch.ticket === ticket)?.prioritised.push(keys);
		},
		cancel: async (ticket) => {
			const batch = batches.find((candidate) => candidate.ticket === ticket);
			if (!batch || batch.cancelled) return false;
			batch.cancelled = true;
			return true;
		},
		ready: (key, url = `thumb://localhost/normal/${encodeURIComponent(key)}.png`) =>
			send(key, { kind: 'ready', key, url }),
		fail: (key, reason = 'could not decode') => send(key, { kind: 'failed', key, reason }),
		skip: (key, why = 'noGenerator') => send(key, { kind: 'skipped', key, why }),
		hold() {
			held = [];
			return {
				release() {
					const waiting = held ?? [];
					held = null;
					for (const resolve of waiting) resolve();
				},
			};
		},
	};
}
