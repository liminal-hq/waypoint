// Exposes typed guest-side wrappers for the trash plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { EmptyFailure } from './bindings/EmptyFailure';
import type { EmptyReport } from './bindings/EmptyReport';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { Flavour } from './bindings/Flavour';
import type { PluginStatus } from './bindings/PluginStatus';
import type { RestoreTarget } from './bindings/RestoreTarget';
import type { TrashError } from './bindings/TrashError';
import type { TrashOutcome } from './bindings/TrashOutcome';
import type { TrashReceipt } from './bindings/TrashReceipt';
import type { TrashedItem } from './bindings/TrashedItem';

export type {
	EmptyFailure,
	EmptyReport,
	FeatureStatus,
	Flavour,
	PluginStatus,
	RestoreTarget,
	TrashError,
	TrashOutcome,
	TrashReceipt,
	TrashedItem,
};

const PREFIX = 'plugin:trash|';

/** The names `getStatus().features` uses. */
export type Feature = 'trash' | 'list' | 'restore' | 'empty' | 'expiry' | 'per-volume';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports which features work on this system, each with a reason when it does not, and which implementation is behind them. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** True if the feature is available. Decide behaviour from the features, never from the platform. */
export function hasFeature(status: PluginStatus, feature: Feature): boolean {
	return status.features.some((entry) => entry.name === feature && entry.available);
}

/** Why a feature is unavailable, or `undefined` when it works. */
export function featureReason(status: PluginStatus, feature: Feature): string | undefined {
	return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
}

/**
 * Moves each absolute path to the trash, as one batch. The result has one outcome per path, in
 * order: a path that fails does not stop the others, and every failure is reported.
 */
export function trash(paths: string[]): Promise<TrashOutcome[]> {
	return cmd<TrashOutcome[]>('trash', { paths });
}

/** Everything in the trash, oldest first. Rejects with a `TrashError` (`unsupported` where `list` is unavailable). */
export function list(): Promise<TrashedItem[]> {
	return cmd<TrashedItem[]>('list');
}

/**
 * Puts an item back where it was trashed from (the default) or at a full path of the caller's choice.
 * Never overwrites: rejects with `originExists` when the place is taken and `originMissingParent`
 * when its folder is gone, so the caller can prompt and try again with `{ kind: 'path', path }`.
 */
export function restore(
	receipt: TrashReceipt,
	target: RestoreTarget = { kind: 'original' },
): Promise<TrashReceipt> {
	return cmd<TrashReceipt>('restore', { receipt, target });
}

/** Removes one item from the trash for good. */
export async function deleteItem(receipt: TrashReceipt): Promise<void> {
	await cmd<void>('delete', { receipt });
}

/**
 * Empties the trash, or with `olderThanDays` only the items trashed that many days ago or earlier.
 * Items that cannot be removed are listed in `failed` and stay in the trash.
 */
export function empty(olderThanDays?: number): Promise<EmptyReport> {
	return cmd<EmptyReport>('empty', { olderThanDays: olderThanDays ?? null });
}

/** True if a rejected value is a `TrashError`. */
export function isTrashError(value: unknown): value is TrashError {
	return typeof value === 'object' && value !== null && typeof (value as TrashError).kind === 'string';
}
