// An in-memory OsAppearanceClient: answers what the test sets and announces changes like the plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AppearancePreferences, OsAppearanceClient } from './osAppearanceClient';

export const NEUTRAL_PREFERENCES: AppearancePreferences = {
	revision: 1,
	colourScheme: 'noPreference',
	accent: null,
	contrast: 'normal',
	reducedMotion: false,
	reducedTransparency: false,
	textScale: 1,
	iconTheme: null,
	sources: {
		colourScheme: null,
		accent: null,
		contrast: null,
		reducedMotion: null,
		reducedTransparency: null,
		textScale: null,
		iconTheme: null,
	},
};

export interface FakeOsAppearance extends OsAppearanceClient {
	/** Changes what is in force as the desktop would: stored, with a higher revision, and announced. */
	change(next: Partial<AppearancePreferences>): AppearancePreferences;
	/** Sends an event as the plugin would, without changing what is in force (a late or repeated one). */
	emit(preferences: AppearancePreferences): void;
	/** Makes `get` reject, as a plugin that is not there or a call that failed would. */
	failGet(): void;
	/** How many listeners are attached now. */
	readonly listenerCount: number;
}

export function createFakeOsAppearanceClient(
	initial: Partial<AppearancePreferences> = {},
): FakeOsAppearance {
	let current: AppearancePreferences = { ...NEUTRAL_PREFERENCES, ...initial };
	let failing = false;
	const listeners = new Set<(preferences: AppearancePreferences) => void>();
	const emit = (preferences: AppearancePreferences): void => {
		for (const listener of [...listeners]) listener(preferences);
	};
	return {
		get: () => (failing ? Promise.reject(new Error('no plugin')) : Promise.resolve(current)),
		onChanged(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		change(next) {
			current = { ...current, ...next, revision: current.revision + 1 };
			emit(current);
			return current;
		},
		emit,
		failGet() {
			failing = true;
		},
		get listenerCount() {
			return listeners.size;
		},
	};
}
