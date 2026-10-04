// Verifies the root element follows the settings and the OS preferences live
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { OsPaletteClient, Palette, PaletteEntry } from '../services/osPaletteClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { NO_OS_APPEARANCE, type OsAppearance, type OsAppearanceSource } from './appearance';
import { prefersReducedMotion } from './motion';
import { resetPaletteReport } from './paletteReport';
import { ThemeRoot } from './ThemeRoot';

function fakeOs(initial: Partial<OsAppearance> = {}) {
	let value: OsAppearance = { ...NO_OS_APPEARANCE, ...initial };
	const listeners = new Set<() => void>();
	const source: OsAppearanceSource = {
		read: () => value,
		subscribe(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
	return {
		source,
		change(next: Partial<OsAppearance>) {
			value = { ...value, ...next };
			for (const listener of listeners) listener();
		},
	};
}

afterEach(() => {
	resetPaletteReport();
	const root = document.documentElement;
	for (const key of Object.keys(root.dataset)) delete root.dataset[key];
	root.removeAttribute('style');
});

describe('ThemeRoot', () => {
	it('sets the OS scheme on the root and follows it when it changes', async () => {
		const os = fakeOs({ scheme: 'light' });
		await act(async () => {
			render(
				<ThemeRoot client={createFakeSettingsClient()} os={os.source}>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		expect(document.documentElement.dataset.theme).toBe('light');
		await act(async () => os.change({ scheme: 'dark', reducedMotion: true }));
		expect(document.documentElement.dataset.theme).toBe('dark');
		expect(document.documentElement.dataset.motion).toBe('reduce');
		expect(prefersReducedMotion()).toBe(true);
	});

	it('follows a setting the moment Rust announces it', async () => {
		const fake = createFakeSettingsClient();
		await act(async () => {
			render(
				<ThemeRoot client={fake} os={fakeOs({ scheme: 'light' }).source}>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		await act(async () => {
			await fake.set({
				...DEFAULT_SETTINGS,
				appearance: { ...DEFAULT_SETTINGS.appearance, mode: 'dark', density: 'spacious' },
				accessibility: { ...DEFAULT_SETTINGS.accessibility, textSize: 130 },
			});
		});
		expect(document.documentElement.dataset).toMatchObject({ theme: 'dark', density: 'spacious' });
		expect(document.documentElement.style.getPropertyValue('--wp-text-scale')).toBe('1.3');
	});
});

describe('ThemeRoot lang and dir', () => {
	const withLocale = (language: string, direction: 'auto' | 'ltr' | 'rtl' = 'auto') => ({
		...DEFAULT_SETTINGS,
		locale: { language, direction },
	});
	const mount = async (fake = createFakeSettingsClient()) => {
		await act(async () => {
			render(
				<ThemeRoot client={fake} os={fakeOs().source}>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		return fake;
	};
	afterEach(() => {
		document.documentElement.removeAttribute('lang');
		document.documentElement.removeAttribute('dir');
	});

	it('sets lang and dir from the chosen language and follows a change live', async () => {
		const fake = await mount(createFakeSettingsClient(withLocale('fr-CA')));
		expect(document.documentElement.lang).toBe('fr-CA');
		expect(document.documentElement.dir).toBe('ltr');
		await act(async () => {
			await fake.set(withLocale('en-CA'));
		});
		expect(document.documentElement.lang).toBe('en-CA');
	});

	it('is right to left for the Arabic pseudo-locale, and the direction setting overrides the language', async () => {
		const fake = await mount(createFakeSettingsClient(withLocale('ar-XB')));
		expect(document.documentElement.lang).toBe('ar-XB');
		expect(document.documentElement.dir).toBe('rtl');
		await act(async () => {
			await fake.set(withLocale('ar-XB', 'ltr'));
		});
		expect(document.documentElement.dir).toBe('ltr');
		await act(async () => {
			await fake.set(withLocale('en-CA', 'rtl'));
		});
		expect(document.documentElement.lang).toBe('en-CA');
		expect(document.documentElement.dir).toBe('rtl');
	});

	it('names the language of the text in force for "system", English where there is no catalogue', async () => {
		await mount();
		expect(document.documentElement.lang).toBe('en-CA');
		expect(document.documentElement.dir).toBe('ltr');
	});
});

describe('ThemeRoot transparency', () => {
	const lit = {
		...DEFAULT_SETTINGS,
		transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true, solidWhenUnfocused: false },
	};
	const root = document.documentElement;

	async function mount(
		settings: typeof DEFAULT_SETTINGS,
		options: { available?: boolean; os?: Partial<OsAppearance> } = {},
	) {
		const os = fakeOs(options.os);
		await act(async () => {
			render(
				<ThemeRoot
					client={createFakeSettingsClient(settings)}
					os={os.source}
					opacityAvailable={() => Promise.resolve(options.available ?? true)}
				>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		return os;
	}

	it('stays solid until the platform says windows can be see-through', async () => {
		await mount(lit, { available: false });
		expect(root.dataset.transparency).toBe('off');
		expect(root.dataset.transparencyReason).toBe('unavailable');
		expect(root.style.getPropertyValue('--wp-alpha-rows')).toBe('');
	});

	it('draws translucent when asked for, writing the opacity of every part on the root', async () => {
		await mount(lit);
		expect(root.dataset.transparency).toBe('on');
		expect(root.dataset.transparencyReason).toBeUndefined();
		// No theme colours in a test, so every part is at least the 70 % fallback.
		const read = (name: string) => Number(root.style.getPropertyValue(name));
		expect(read('--wp-alpha-title-bar')).toBe(0.82);
		expect(read('--wp-alpha-rows')).toBe(0.9);
		expect(read('--wp-alpha-sidebar')).toBe(0.94);
		expect(read('--wp-alpha-content')).toBe(1);
		expect(read('--wp-alpha-menu')).toBe(1);
		expect(root.dataset.menus).toBe('solid');
	});

	it('goes solid and takes the opacity off under high contrast and under reduced transparency', async () => {
		const os = await mount(lit, { os: { highContrast: true } });
		expect(root.dataset.transparency).toBe('off');
		expect(root.dataset.transparencyReason).toBe('high-contrast');
		expect(root.style.getPropertyValue('--wp-alpha-title-bar')).toBe('');
		await act(async () => os.change({ highContrast: false, reducedTransparency: true }));
		expect(root.dataset.transparencyReason).toBe('reduced-transparency');
		await act(async () => os.change({ reducedTransparency: false }));
		expect(root.dataset.transparency).toBe('on');
	});

	it('marks menus translucent when they are, and follows the settings live', async () => {
		const client = createFakeSettingsClient(lit);
		await act(async () => {
			render(
				<ThemeRoot
					client={client}
					os={fakeOs().source}
					opacityAvailable={() => Promise.resolve(true)}
				>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		await act(async () =>
			client.change({
				...lit,
				transparency: { ...lit.transparency, menus: true, menuOpacity: 80, opacity: 60 },
			}),
		);
		expect(root.dataset.menus).toBe('translucent');
		expect(Number(root.style.getPropertyValue('--wp-alpha-menu'))).toBe(0.8);
		expect(Number(root.style.getPropertyValue('--wp-alpha-title-bar'))).toBe(0.7);
		await act(async () =>
			client.change({ ...lit, transparency: { ...lit.transparency, enabled: false } }),
		);
		expect(root.dataset.transparency).toBe('off');
		expect(root.dataset.menus).toBeUndefined();
	});

	it('goes solid when the window loses focus and back when it returns, if asked to', async () => {
		await mount({ ...lit, transparency: { ...lit.transparency, solidWhenUnfocused: true } });
		await act(async () => {
			globalThis.dispatchEvent(new Event('focus'));
		});
		expect(root.dataset.transparency).toBe('on');
		await act(async () => {
			globalThis.dispatchEvent(new Event('blur'));
		});
		expect(root.dataset.transparency).toBe('off');
		expect(root.dataset.transparencyReason).toBe('unfocused');
		await act(async () => {
			globalThis.dispatchEvent(new Event('focus'));
		});
		expect(root.dataset.transparency).toBe('on');
	});
});

describe('ThemeRoot and the OS palette', () => {
	const entry = (colour: string): PaletteEntry => ({
		colour,
		source: 'gtkTheme',
		reason: null,
		detail: null,
	});
	const none: PaletteEntry = { colour: null, source: null, reason: 'sourceMissing', detail: null };
	const palette = (revision: number, window: string, text: string): Palette => ({
		revision,
		status: { available: true, source: 'gtkTheme', reason: null, detail: null },
		windowBackground: entry(window),
		windowForeground: entry(text),
		viewBackground: entry(window),
		viewForeground: entry(text),
		surfaceBackground: none,
		selectionBackground: none,
		selectionForeground: none,
		border: none,
		focus: none,
		warning: none,
		error: none,
		success: none,
	});
	function fakePalette(initial: Palette | null) {
		const listeners = new Set<(next: Palette) => void>();
		const client: OsPaletteClient = {
			get: () => (initial ? Promise.resolve(initial) : Promise.reject(new Error('no palette'))),
			onChanged(listener) {
				listeners.add(listener);
				return () => listeners.delete(listener);
			},
		};
		return { client, push: (next: Palette) => listeners.forEach((listener) => listener(next)) };
	}
	const asked = {
		...DEFAULT_SETTINGS,
		appearance: { ...DEFAULT_SETTINGS.appearance, matchSystemColours: true },
	};
	const mount = async (
		fake: ReturnType<typeof fakePalette>,
		settings = createFakeSettingsClient(asked),
	) => {
		await act(async () => {
			render(
				<ThemeRoot
					client={settings}
					os={fakeOs({ scheme: 'light' }).source}
					osPalette={fake.client}
				>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		return settings;
	};
	const root = document.documentElement;

	it('draws nothing of the palette while the option is off', async () => {
		await mount(fakePalette(palette(1, '#242424', '#ffffff')), createFakeSettingsClient());
		expect(root.style.getPropertyValue('--wp-solid-window')).toBe('');
		expect(root.dataset.palette).toBeUndefined();
		expect(root.dataset.theme).toBe('light');
	});

	it('draws the palette, with its variant, when the option is on', async () => {
		await mount(fakePalette(palette(1, '#242424', '#ffffff')));
		expect(root.style.getPropertyValue('--wp-solid-window')).toBe('#242424');
		expect(root.dataset.palette).toBe('system');
		expect(root.dataset.theme).toBe('dark');
	});

	it('follows a change of the OS palette live, and ignores an older revision', async () => {
		const fake = fakePalette(palette(2, '#242424', '#ffffff'));
		await mount(fake);
		await act(async () => fake.push(palette(3, '#fafafa', '#2e3436')));
		expect(root.style.getPropertyValue('--wp-solid-window')).toBe('#fafafa');
		expect(root.dataset.theme).toBe('light');
		await act(async () => fake.push(palette(1, '#000000', '#ffffff')));
		expect(root.style.getPropertyValue('--wp-solid-window')).toBe('#fafafa');
	});

	it('keeps Waypoint’s own colours where the plugin cannot answer', async () => {
		await mount(fakePalette(null));
		expect(root.style.getPropertyValue('--wp-solid-window')).toBe('');
		expect(root.dataset.theme).toBe('light');
	});

	it('switches off live when the option is turned off, and back on', async () => {
		const settings = await mount(fakePalette(palette(1, '#242424', '#ffffff')));
		await act(async () => {
			await settings.set(DEFAULT_SETTINGS);
		});
		expect(root.style.getPropertyValue('--wp-solid-window')).toBe('');
		expect(root.dataset.palette).toBeUndefined();
		await act(async () => {
			await settings.set(asked);
		});
		expect(root.style.getPropertyValue('--wp-solid-window')).toBe('#242424');
	});
});
