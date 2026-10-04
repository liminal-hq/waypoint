// Exposes typed guest-side wrappers for the secrets plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { Flavour } from './bindings/Flavour';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Reason } from './bindings/Reason';
import type { SecretId } from './bindings/SecretId';
import type { SecretKind } from './bindings/SecretKind';
import type { SecretsError } from './bindings/SecretsError';

export type { FeatureStatus, Flavour, PluginStatus, Reason, SecretId, SecretKind, SecretsError };

const PREFIX = 'plugin:secrets|';

/** The names `getStatus().features` uses. */
export type Feature = 'store' | 'fetch' | 'delete';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports which features work on this system, each with a typed reason when it does not, and which keyring is behind them. It never prompts to unlock the keyring. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/** True if the feature is available. Decide behaviour from the features, never from the platform. */
export function hasFeature(status: PluginStatus, feature: Feature): boolean {
	return status.features.some((entry) => entry.name === feature && entry.available);
}

/** Why a feature is unavailable, as a code to branch on (`no-keyring`, `locked`, …), or `undefined` when it works. */
export function featureReason(status: PluginStatus, feature: Feature): Reason | undefined {
	return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
}

/** A sentence that explains why a feature is unavailable, or `undefined` when it works. */
export function featureMessage(status: PluginStatus, feature: Feature): string | undefined {
	return status.features.find((entry) => entry.name === feature)?.message ?? undefined;
}

/**
 * Stores a secret, replacing one with the same id. The value crosses the bridge once, is neither
 * logged nor kept by the plugin and is not returned. `label` is what the keyring's own manager shows.
 * Rejects with a `SecretsError`.
 */
export async function store(id: SecretId, secret: string, label?: string): Promise<void> {
	await cmd<void>('store', { id, secret, label });
}

/**
 * Reads a secret back, or `null` when there is none. Needs the separate `secrets:allow-fetch`
 * permission, which the plugin's default set leaves out: prefer to use a secret in Rust.
 */
export function fetch(id: SecretId): Promise<string | null> {
	return cmd<string | null>('fetch', { id });
}

/** Whether a secret exists, without reading it. */
export function exists(id: SecretId): Promise<boolean> {
	return cmd<boolean>('exists', { id });
}

/** Deletes a secret; true when there was one. */
export function remove(id: SecretId): Promise<boolean> {
	return cmd<boolean>('delete', { id });
}

/** Deletes every kind of secret of one service and account and returns how many there were. */
export function removeAccount(service: string, account: string): Promise<number> {
	return cmd<number>('delete_account', { service, account });
}

/** True if a rejected value is a `SecretsError`. */
export function isSecretsError(value: unknown): value is SecretsError {
	return (
		typeof value === 'object' && value !== null && typeof (value as SecretsError).kind === 'string'
	);
}
