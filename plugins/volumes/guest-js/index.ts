// Exposes typed guest-side wrappers for the volumes plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { Flavour } from './bindings/Flavour';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Reason } from './bindings/Reason';
import type { RememberOutcome } from './bindings/RememberOutcome';
import type { Unlocked } from './bindings/Unlocked';
import type { Volume } from './bindings/Volume';
import type { VolumeKind } from './bindings/VolumeKind';
import type { VolumesChanged } from './bindings/VolumesChanged';
import type { VolumesError } from './bindings/VolumesError';

export type {
	FeatureStatus,
	Flavour,
	PluginStatus,
	Reason,
	RememberOutcome,
	Unlocked,
	Volume,
	VolumeKind,
	VolumesChanged,
	VolumesError,
};

const PREFIX = 'plugin:volumes|';

/** The event name of `onChanged`. */
export const CHANGED_EVENT = 'volumes://changed';

/** The names `getStatus().features` uses. */
export type Feature = 'list' | 'mount' | 'unmount' | 'eject' | 'unlock' | 'watch' | 'remember';

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

/** Why a feature is unavailable, as a code to branch on (`no-system-bus`, `udisks2-missing`, `flatpak-sandbox`, …), or `undefined` when it works. */
export function featureReason(status: PluginStatus, feature: Feature): Reason | undefined {
	return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
}

/** A sentence that explains why a feature is unavailable, or `undefined` when it works. */
export function featureMessage(status: PluginStatus, feature: Feature): string | undefined {
	return status.features.find((entry) => entry.name === feature)?.message ?? undefined;
}

/**
 * Every volume now. Mounted local volumes are measured within a timeout, so `free` is `null` for one
 * that did not answer. Network volumes are not measured unless `measure` is true; use `refreshSpace`
 * to measure one.
 */
export function list(measure = false): Promise<Volume[]> {
	return cmd<Volume[]>('list', { measure });
}

/** Measures one volume, network or not, within the timeout, and returns it with its `total` and `free` when they were found. */
export function refreshSpace(id: string): Promise<Volume> {
	return cmd<Volume>('refresh_space', { id });
}

/** Mounts a volume and returns its mount point. Rejects with a `VolumesError`. */
export function mount(id: string): Promise<string> {
	return cmd<string>('mount', { id });
}

/** Unmounts a volume. Rejects with `{ kind: 'busy', by }` while something holds it. */
export async function unmount(id: string): Promise<void> {
	await cmd<void>('unmount', { id });
}

/** Unmounts everything on the volume's drive and ejects it, powering the drive off when it allows. */
export async function eject(id: string): Promise<void> {
	await cmd<void>('eject', { id });
}

/**
 * Unlocks an encrypted volume and returns the id of the volume that appears (mount it next). The
 * passphrase is sent once and neither logged nor kept by the plugin; a wrong one rejects with
 * `wrongPassphrase`. With `remember` true the passphrase is also handed to the app's own store (a
 * keyring), when the `remember` feature works: `remember` in the result says whether that
 * happened, or why not. A passphrase that could not be kept does not undo the unlock.
 */
export function unlock(id: string, passphrase: string, remember = false): Promise<Unlocked> {
	return cmd<Unlocked>('unlock', { id, passphrase, remember });
}

/** Forgets the passphrase kept for an encrypted volume (`Volume.remembered`); true when there was one. */
export function forget(id: string): Promise<boolean> {
	return cmd<boolean>('forget', { id });
}

/**
 * Calls `handler` with the whole list each time it changes. Revisions only grow: ignore an event
 * whose `revision` is not above the last one you have seen.
 */
export function onChanged(handler: (event: VolumesChanged) => void): Promise<UnlistenFn> {
	return listen<VolumesChanged>(CHANGED_EVENT, (event) => handler(event.payload));
}

/** True if a rejected value is a `VolumesError`. */
export function isVolumesError(value: unknown): value is VolumesError {
	return (
		typeof value === 'object' && value !== null && typeof (value as VolumesError).kind === 'string'
	);
}
