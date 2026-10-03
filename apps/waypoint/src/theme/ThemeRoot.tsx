// Keeps the window's root element in step with the OS preferences and the settings
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getStatus as windowEffectsStatus, hasFeature } from '@liminal-hq/plugin-window-effects';
import { useEffect, useState, type ReactNode } from 'react';
import { DEFAULT_SETTINGS, type SettingsClient } from '../services/settingsClient';
import type { OsPaletteClient, Palette } from '../services/osPaletteClient';
import { createTauriOsAppearanceClient } from '../services/tauriOsAppearanceClient';
import { createTauriOsPaletteClient } from '../services/tauriOsPaletteClient';
import { createTauriSettingsClient } from '../services/tauriSettingsClient';
import {
	applyAppearance,
	resolveAppearance,
	webOsAppearance,
	type OsAppearanceSource,
} from './appearance';
import { reportPalette } from './paletteReport';
import { pluginOsAppearance } from './pluginAppearance';
import { applyTransparency } from './transparencyDom';

interface ThemeRootProps {
	/** The settings plugin's client; the real one unless a test supplies its own. */
	client?: SettingsClient;
	/** Where the OS preferences come from; the `system-appearance` plugin over the webview's media queries unless supplied. */
	os?: OsAppearanceSource;
	/** Where the OS colour palette comes from; the `system-appearance` plugin unless supplied. */
	osPalette?: OsPaletteClient;
	/** Whether the platform can make windows see-through: the window effects plugin's `opacity` feature unless supplied. */
	opacityAvailable?: () => Promise<boolean>;
	children: ReactNode;
}

async function pluginOpacityAvailable(): Promise<boolean> {
	return hasFeature(await windowEffectsStatus(), 'opacity');
}

/** Whether this window is in front: the document's focus, followed through the window's own events. */
function useWindowInFront(): boolean {
	const [inFront, setInFront] = useState(true);
	useEffect(() => {
		const sync = (): void => setInFront(document.hasFocus());
		const onFocus = (): void => setInFront(true);
		const onBlur = (): void => setInFront(false);
		globalThis.addEventListener('focus', onFocus);
		globalThis.addEventListener('blur', onBlur);
		sync();
		return () => {
			globalThis.removeEventListener('focus', onFocus);
			globalThis.removeEventListener('blur', onBlur);
		};
	}, []);
	return inFront;
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
export function ThemeRoot({ client, os, osPalette, opacityAvailable, children }: ThemeRootProps) {
	const [settingsClient] = useState(() => client ?? createTauriSettingsClient());
	const [source] = useState(
		() => os ?? pluginOsAppearance(createTauriOsAppearanceClient(), webOsAppearance()),
	);
	const [paletteClient] = useState(() => osPalette ?? createTauriOsPaletteClient());
	const [palette, setPalette] = useState<Palette | null>(null);
	const [settings, setSettings] = useState(DEFAULT_SETTINGS);
	const [touchPointer, setTouchPointer] = useState(false);
	const [osVersion, setOsVersion] = useState(0);
	const [canBeSeeThrough, setCanBeSeeThrough] = useState<boolean | null>(null);
	const inFront = useWindowInFront();

	// What the platform can do decides whether transparency may draw at all (A60): until it has
	// said, and when it cannot say, the window stays solid.
	useEffect(() => {
		let live = true;
		(opacityAvailable ?? pluginOpacityAvailable)().then(
			(available) => live && setCanBeSeeThrough(available),
			() => live && setCanBeSeeThrough(false),
		);
		return () => {
			live = false;
		};
	}, [opacityAvailable]);

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

	// The palette is asked for whether or not the option is on, so the Appearance page knows whether
	// to offer it. It listens before it reads and keeps only the newest revision.
	useEffect(() => {
		let live = true;
		let revision = 0;
		const accept = (next: Palette): void => {
			if (live && next.revision > revision) {
				revision = next.revision;
				setPalette(next);
			}
		};
		const stop = paletteClient.onChanged(accept);
		paletteClient.get().then(accept, () => undefined);
		return () => {
			live = false;
			stop();
		};
	}, [paletteClient]);

	useEffect(() => source.subscribe(() => setOsVersion((n) => n + 1)), [source]);

	// The last pointer to touch the window decides touch mode's "Auto".
	useEffect(() => {
		const onPointer = (event: PointerEvent): void => setTouchPointer(event.pointerType === 'touch');
		globalThis.addEventListener('pointerdown', onPointer, true);
		return () => globalThis.removeEventListener('pointerdown', onPointer, true);
	}, []);

	useEffect(() => {
		const root = document.documentElement;
		const look = resolveAppearance(settings, source.read(), {
			touchPointer,
			systemLanguage: systemLanguage(),
			hasFinePointer: hasFinePointer(),
			focused: inFront,
			opacityAvailable: canBeSeeThrough,
			palette,
		});
		applyAppearance(root, look);
		reportPalette({
			available: look.palette.available,
			state: look.palette.state,
			lifted: look.palette.state === 'applied' ? look.palette.lifted : [],
		});
		// After the attributes, so the floor is worked out from the colours of the scheme now in force.
		applyTransparency(root, look.transparency === 'on', settings.transparency);
	}, [settings, source, touchPointer, osVersion, inFront, canBeSeeThrough, palette]);

	return <>{children}</>;
}
