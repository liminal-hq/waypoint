// Exposes typed guest-side wrappers for the window-effects plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import type { EffectKind } from './bindings/EffectKind';
import type { Effects } from './bindings/Effects';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { Flavour } from './bindings/Flavour';
import type { Insets } from './bindings/Insets';
import type { PluginStatus } from './bindings/PluginStatus';
import type { Reason } from './bindings/Reason';
import type { Rect } from './bindings/Rect';
import type { WindowEffectsError } from './bindings/WindowEffectsError';

export type {
	EffectKind,
	Effects,
	FeatureStatus,
	Flavour,
	Insets,
	PluginStatus,
	Reason,
	Rect,
	WindowEffectsError,
};

const PREFIX = 'plugin:window-effects|';

/** The names `getStatus().features` uses. */
export type Feature = 'opacity' | 'blur' | 'mica' | 'acrylic' | 'shadowInset';

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

/** Why a feature is unavailable, as a code to branch on (`compositor-has-no-blur`, `needs-windows-11`, `x11-no-compositor`, …), or `undefined` when it works. */
export function featureReason(status: PluginStatus, feature: Feature): Reason | undefined {
	return status.features.find((entry) => entry.name === feature)?.reason ?? undefined;
}

/** A sentence that explains why a feature is unavailable, or `undefined` when it works. */
export function featureMessage(status: PluginStatus, feature: Feature): string | undefined {
	return status.features.find((entry) => entry.name === feature)?.message ?? undefined;
}

/**
 * Puts an effect behind the window with that label, replacing the one it had. How see-through the
 * window is stays the page's own alpha: the effect shows only where the page is transparent. Apply
 * again when the theme changes (`dark`) or, with a `region`, when the window is resized. Rejects
 * with a `WindowEffectsError`: `{ kind: 'unsupported', reason, message }` when this system cannot do it.
 */
export async function apply(label: string, effects: Effects): Promise<void> {
	await cmd<void>('apply', { label, effects });
}

/** Takes the window's effect away. */
export async function clear(label: string): Promise<void> {
	await cmd<void>('clear', { label });
}

/** Tells the compositor how much of the window, in logical pixels, is shadow or invisible border, so a tiled window sits flush. All zeros puts it back. */
export async function setShadowInset(label: string, insets: Insets): Promise<void> {
	await cmd<void>('set_shadow_inset', { label, insets });
}

/** True if a rejected value is a `WindowEffectsError`. */
export function isWindowEffectsError(value: unknown): value is WindowEffectsError {
	return (
		typeof value === 'object' &&
		value !== null &&
		typeof (value as WindowEffectsError).kind === 'string'
	);
}
