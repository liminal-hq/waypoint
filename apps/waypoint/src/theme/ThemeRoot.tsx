// Keeps the window's root element in step with the OS preferences and the settings
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState, type ReactNode } from 'react';
import { DEFAULT_SETTINGS, type SettingsClient } from '../services/settingsClient';
import { createTauriSettingsClient } from '../services/tauriSettingsClient';
import {
	applyAppearance,
	resolveAppearance,
	webOsAppearance,
	type OsAppearanceSource,
} from './appearance';

interface ThemeRootProps {
	/** The settings plugin's client; the real one unless a test supplies its own. */
	client?: SettingsClient;
	/** Where the OS preferences come from; the webview's media queries unless supplied. */
	os?: OsAppearanceSource;
	children: ReactNode;
}

function systemLanguage(): string {
	const tag = globalThis.navigator?.language;
	return tag && /^[a-zA-Z]{2,3}(-[a-zA-Z0-9]+)*$/.test(tag) ? tag : 'en-CA';
}

function hasFinePointer(): boolean {
	return (
		typeof globalThis.matchMedia !== 'function' ||
		globalThis.matchMedia('(any-pointer: fine)').matches
	);
}

/**
 * Mounted once per window. It follows the settings (events from Rust, the one owner; the defaults
 * until the first answer) and the OS preferences, and writes the result on the root so no media
 * query is the only signal (A58). It renders its children untouched.
 */
export function ThemeRoot({ client, os, children }: ThemeRootProps) {
	const [settingsClient] = useState(() => client ?? createTauriSettingsClient());
	const [source] = useState(() => os ?? webOsAppearance());
	const [settings, setSettings] = useState(DEFAULT_SETTINGS);
	const [touchPointer, setTouchPointer] = useState(false);
	const [osVersion, setOsVersion] = useState(0);

	useEffect(() => {
		let live = true;
		let revision = -1;
		settingsClient.snapshot().then(
			(snapshot) => {
				if (live && snapshot.revision > revision) {
					revision = snapshot.revision;
					setSettings(snapshot.settings);
				}
			},
			() => undefined,
		);
		const stop = settingsClient.onChanged((snapshot) => {
			if (snapshot.revision > revision) {
				revision = snapshot.revision;
				setSettings(snapshot.settings);
			}
		});
		return () => {
			live = false;
			stop();
		};
	}, [settingsClient]);

	useEffect(() => source.subscribe(() => setOsVersion((n) => n + 1)), [source]);

	// The last pointer to touch the window decides touch mode's "Auto".
	useEffect(() => {
		const onPointer = (event: PointerEvent): void => setTouchPointer(event.pointerType === 'touch');
		globalThis.addEventListener('pointerdown', onPointer, true);
		return () => globalThis.removeEventListener('pointerdown', onPointer, true);
	}, []);

	useEffect(() => {
		applyAppearance(
			document.documentElement,
			resolveAppearance(settings, source.read(), {
				touchPointer,
				systemLanguage: systemLanguage(),
				hasFinePointer: hasFinePointer(),
			}),
		);
	}, [settings, source, touchPointer, osVersion]);

	return <>{children}</>;
}
