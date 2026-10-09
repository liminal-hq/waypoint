// Exposes typed guest-side wrappers for the elevate plugin: the status only, never a launch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { Flavour } from './bindings/Flavour';
import type { PluginStatus } from './bindings/PluginStatus';

export type { FeatureStatus, Flavour, PluginStatus };

const PREFIX = 'plugin:elevate|';

/** The names `getStatus().features` uses. */
export type Feature = 'elevate';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/**
 * Reports whether a helper can be started with administrator rights on this system, with a reason for
 * the person when it cannot. Starting one is not exposed to the page on purpose: only Rust code can.
 */
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
