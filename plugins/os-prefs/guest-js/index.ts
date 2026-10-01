// Exposes typed guest-side wrappers for the os-prefs plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AnimatorDurationScaleResponse } from './bindings/AnimatorDurationScaleResponse';
import type { FeatureStatus } from './bindings/FeatureStatus';
import type { PluginStatus } from './bindings/PluginStatus';
import type { TimeFormat } from './bindings/TimeFormat';
import type { TimeFormatSource } from './bindings/TimeFormatSource';

const PREFIX = 'plugin:os-prefs|';

/** Event emitted to all windows with the new `TimeFormat` when the user flips the 12/24-hour setting. Desktop only. */
export const TIME_FORMAT_CHANGED_EVENT = 'os-prefs://time-format-changed';

function cmd<T>(name: string, args?: Record<string, unknown>): Promise<T> {
	return invoke<T>(`${PREFIX}${name}`, args);
}

/** Reports which features work on this system, with a reason for each that does not or is degraded. */
export function getStatus(): Promise<PluginStatus> {
	return cmd<PluginStatus>('get_status');
}

/**
 * Reads the user's 12/24-hour clock preference. `source` says where the answer came from; the
 * value `default` means nothing could be read and `is24Hour` is only a guess.
 */
export function getTimeFormat(): Promise<TimeFormat> {
	return cmd<TimeFormat>('get_time_format');
}

/** Android's Developer Options "Animator duration scale" (default 1). Desktop and iOS always report 1. */
export function getAnimatorDurationScale(): Promise<AnimatorDurationScaleResponse> {
	return cmd<AnimatorDurationScaleResponse>('get_animator_duration_scale');
}

/** Opens the OS notification settings screen for this app. Android only; a no-op elsewhere. */
export async function openNotificationSettings(): Promise<void> {
	await cmd<void>('open_notification_settings');
}

/**
 * Subscribes to changes of the 12/24-hour setting and resolves to a function that unsubscribes.
 * Events are pushed on GNOME, Cinnamon, Windows and macOS; `getStatus()` reports whether this
 * system does (the `timeFormatWatch` feature).
 */
export function onTimeFormatChanged(callback: (_format: TimeFormat) => void): Promise<() => void> {
	return listen<TimeFormat>(TIME_FORMAT_CHANGED_EVENT, (event) => callback(event.payload));
}

/**
 * The `hourCycle` to give `Intl.DateTimeFormat` for a reading: `h23` for 24-hour and `h12` for
 * 12-hour. A guessed reading (`source` of `default`) gives `undefined`, which leaves the choice to
 * `Intl` and the locale.
 */
export function hourCycleOf(format: TimeFormat): 'h23' | 'h12' | undefined {
	if (format.source === 'default') {
		return undefined;
	}
	return format.is24Hour ? 'h23' : 'h12';
}

export type {
	AnimatorDurationScaleResponse,
	FeatureStatus,
	PluginStatus,
	TimeFormat,
	TimeFormatSource,
};
