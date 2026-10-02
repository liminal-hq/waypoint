// Exposes typed guest-side wrappers for the thumbnails plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Channel, invoke } from '@tauri-apps/api/core';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { Flavour } from './bindings/Flavour';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Reason } from './bindings/Reason';
import type { ReasonKind } from './bindings/ReasonKind';
import type { SkipWhy } from './bindings/SkipWhy';
import type { ThumbEvent } from './bindings/ThumbEvent';
import type { ThumbRequest } from './bindings/ThumbRequest';
import type { ThumbSize } from './bindings/ThumbSize';

export type {
	FeatureStatus,
	Flavour,
	PluginStatus,
	Reason,
	ReasonKind,
	SkipWhy,
	ThumbEvent,
	ThumbRequest,
	ThumbSize,
};

/** The handle of one `request`, to cancel or reprioritise it. */
export type Ticket = number;

const PREFIX = 'plugin:thumbnails|';

/** The names `getStatus().features` uses. */
export type Feature = 'cache' | 'builtin' | 'external' | 'shell';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports which features work on this system, each with a typed reason when it does not, and which implementation is behind them. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** True if the feature is available. Decide behaviour from the features, never from the platform. */
export function hasFeature(status: PluginStatus, feature: Feature): boolean {
	return status.features.some((entry) => entry.name === feature && entry.available);
}

/** Why a feature is unavailable, or `undefined` when it works. */
export function featureReason(status: PluginStatus, feature: Feature): Reason | undefined {
	return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
}

/**
 * Asks for thumbnails. Each result arrives through `onEvent` as it is made, the newest request
 * first; a key requested twice is one job. A `ready` event's `url` names a cache entry and is
 * loaded like any image address; it never contains a path. Returns the ticket to cancel or
 * reprioritise with.
 */
export function request(
	items: ThumbRequest[],
	onEvent: (event: ThumbEvent) => void,
): Promise<Ticket> {
	const channel = new Channel<ThumbEvent>();
	channel.onmessage = onEvent;
	return cmd<Ticket>('request', { items, onReady: channel });
}

/** Withdraws a request: nothing more is sent to it, and its unstarted work is dropped. True if the ticket was still known. */
export function cancel(ticket: Ticket): Promise<boolean> {
	return cmd<boolean>('cancel', { ticket });
}

/** Moves the request's pending items for `keys` to the front of the queue, in the order given. Call it when the visible rows change. */
export async function prioritise(ticket: Ticket, keys: string[]): Promise<void> {
	await cmd<void>('prioritise', { ticket, keys });
}
