// The OS preferences from the system-appearance plugin, with the webview's media queries behind them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AppearancePreferences, OsAppearanceClient } from '../services/osAppearanceClient';
import type { OsAppearance, OsAppearanceSource } from './appearance';

/** The largest OS text scale the page will apply; a bad answer must not make the window unusable. */
export const MAX_OS_TEXT_SCALE = 3;

/** A text scale the page can use: finite, and from 1 (the normal size) to `MAX_OS_TEXT_SCALE`. */
export function clampTextScale(scale: number): number {
	if (!Number.isFinite(scale)) return 1;
	return Math.min(MAX_OS_TEXT_SCALE, Math.max(1, scale));
}

/** Lays what the plugin answered over `base`: a preference with no source stays as the media query said. */
export function mergePreferences(base: OsAppearance, prefs: AppearancePreferences): OsAppearance {
	const { sources } = prefs;
	return {
		scheme:
			sources.colourScheme !== null && prefs.colourScheme !== 'noPreference'
				? prefs.colourScheme
				: base.scheme,
		highContrast: sources.contrast !== null ? prefs.contrast === 'more' : base.highContrast,
		reducedMotion: sources.reducedMotion !== null ? prefs.reducedMotion : base.reducedMotion,
		reducedTransparency:
			sources.reducedTransparency !== null ? prefs.reducedTransparency : base.reducedTransparency,
		textScale: sources.textScale !== null ? clampTextScale(prefs.textScale) : base.textScale,
		accent: sources.accent !== null ? prefs.accent : base.accent,
	};
}

/**
 * The plugin's answer where it has one, the `fallback` (the media queries) for every preference
 * it cannot say, and all of the fallback until its first answer or if it never comes. It listens
 * before it reads, so a change made while starting is not lost, and keeps only the newest revision.
 */
export function pluginOsAppearance(
	client: OsAppearanceClient,
	fallback: OsAppearanceSource,
): OsAppearanceSource {
	let latest: AppearancePreferences | null = null;
	const listeners = new Set<() => void>();
	let stopClient: (() => void) | null = null;
	let stopFallback: (() => void) | null = null;
	let epoch = 0;

	const notify = (): void => {
		for (const listener of [...listeners]) listener();
	};

	const start = (): void => {
		const mine = ++epoch;
		const accept = (prefs: AppearancePreferences): void => {
			if (mine !== epoch || (latest && prefs.revision <= latest.revision)) return;
			latest = prefs;
			notify();
		};
		stopFallback = fallback.subscribe(notify);
		stopClient = client.onChanged(accept);
		client.get().then(accept, () => undefined);
	};

	const stop = (): void => {
		epoch += 1;
		stopClient?.();
		stopFallback?.();
		stopClient = null;
		stopFallback = null;
	};

	return {
		read() {
			const base = fallback.read();
			return latest ? mergePreferences(base, latest) : base;
		},
		subscribe(listener) {
			if (listeners.size === 0) start();
			listeners.add(listener);
			return () => {
				if (!listeners.delete(listener)) return;
				if (listeners.size === 0) stop();
			};
		},
	};
}
