// Verifies how the settings and the OS preferences become a window's look
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { EMBER } from './accent';
import {
	applyAppearance,
	directionFor,
	NO_OS_APPEARANCE,
	resolveAppearance,
	type OsAppearance,
} from './appearance';

const options = { touchPointer: false, systemLanguage: 'en-CA', hasFinePointer: true };
const os = (change: Partial<OsAppearance>): OsAppearance => ({ ...NO_OS_APPEARANCE, ...change });
const withAppearance = (change: Partial<typeof DEFAULT_SETTINGS.appearance>) => ({
	...DEFAULT_SETTINGS,
	appearance: { ...DEFAULT_SETTINGS.appearance, ...change },
});
const withAccess = (change: Partial<typeof DEFAULT_SETTINGS.accessibility>) => ({
	...DEFAULT_SETTINGS,
	accessibility: { ...DEFAULT_SETTINGS.accessibility, ...change },
});

describe('resolveAppearance transparency', () => {
	const lit = {
		...DEFAULT_SETTINGS,
		transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true },
	};

	it('says why a window that is asked to be translucent is solid', () => {
		const look = (change: Partial<typeof options> & Record<string, unknown>, o = os({})) =>
			resolveAppearance(lit, o, { ...options, ...change });
		expect(look({}).transparencyReason).toBeNull();
		expect(look({ opacityAvailable: null })).toMatchObject({
			transparency: 'off',
			transparencyReason: 'unavailable',
		});
		expect(look({ focused: false })).toMatchObject({
			transparency: 'off',
			transparencyReason: 'unfocused',
		});
		expect(look({}, os({ highContrast: true })).transparencyReason).toBe('high-contrast');
		expect(look({}, os({ reducedTransparency: true })).transparencyReason).toBe(
			'reduced-transparency',
		);
		expect(resolveAppearance(DEFAULT_SETTINGS, os({}), options).transparencyReason).toBeNull();
	});
});

describe('resolveAppearance', () => {
	it('follows the OS scheme by default and a forced mode otherwise', () => {
		expect(resolveAppearance(DEFAULT_SETTINGS, os({ scheme: 'dark' }), options).theme).toBe('dark');
		expect(resolveAppearance(DEFAULT_SETTINGS, os({ scheme: null }), options).theme).toBeNull();
		expect(
			resolveAppearance(withAppearance({ mode: 'light' }), os({ scheme: 'dark' }), options).theme,
		).toBe('light');
	});

	it('follows the OS for high contrast, motion and transparency unless forced', () => {
		const loud = os({ highContrast: true, reducedMotion: true, reducedTransparency: true });
		const followed = resolveAppearance(DEFAULT_SETTINGS, loud, options);
		expect([followed.contrast, followed.motion]).toEqual(['high', 'reduce']);
		const forcedOff = resolveAppearance(
			withAccess({ highContrast: 'off', reducedMotion: 'off', reducedTransparency: 'off' }),
			loud,
			options,
		);
		expect([forcedOff.contrast, forcedOff.motion]).toEqual(['normal', 'full']);
		const forcedOn = resolveAppearance(
			withAccess({ highContrast: 'on', reducedMotion: 'on' }),
			os({}),
			options,
		);
		expect([forcedOn.contrast, forcedOn.motion]).toEqual(['high', 'reduce']);
	});

	it('switches transparency off under high contrast or reduced transparency', () => {
		const on = {
			...DEFAULT_SETTINGS,
			transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true },
		};
		expect(resolveAppearance(on, os({}), options).transparency).toBe('on');
		expect(resolveAppearance(on, os({ highContrast: true }), options).transparency).toBe('off');
		expect(resolveAppearance(on, os({ reducedTransparency: true }), options).transparency).toBe(
			'off',
		);
		expect(resolveAppearance(DEFAULT_SETTINGS, os({}), options).transparency).toBe('off');
	});

	it('takes the larger of the setting and the OS text scale', () => {
		expect(
			resolveAppearance(withAccess({ textSize: 115 }), os({ textScale: 1 }), options).textScale,
		).toBe(1.15);
		expect(
			resolveAppearance(withAccess({ textSize: 100 }), os({ textScale: 1.25 }), options).textScale,
		).toBe(1.25);
	});

	it('turns touch mode on when asked, or in Auto after a touch or with no fine pointer', () => {
		const touch = (access: 'off' | 'auto' | 'on', o: Partial<typeof options>) =>
			resolveAppearance(withAccess({ touchMode: access }), os({}), { ...options, ...o }).touch;
		expect(touch('off', { touchPointer: true })).toBe(false);
		expect(touch('on', {})).toBe(true);
		expect(touch('auto', {})).toBe(false);
		expect(touch('auto', { touchPointer: true })).toBe(true);
		expect(touch('auto', { hasFinePointer: false })).toBe(true);
	});

	it('leaves the brand accent to the tokens and sets one the person chose, with text at 4.5:1', () => {
		expect(resolveAppearance(DEFAULT_SETTINGS, os({ scheme: 'dark' }), options).accent).toBeNull();
		const custom = resolveAppearance(
			withAppearance({ accent: { kind: 'custom', hex: '#0f766e' } }),
			os({ scheme: 'light' }),
			options,
		);
		expect(custom.accent?.fill).toBeDefined();
		const fromOs = resolveAppearance(
			withAppearance({ accent: { kind: 'os' } }),
			os({ scheme: 'dark', accent: null }),
			options,
		);
		expect(fromOs.accent).toEqual(EMBER.dark);
		// High contrast brings its own accent.
		expect(
			resolveAppearance(
				withAppearance({ accent: { kind: 'custom', hex: '#0f766e' } }),
				os({ highContrast: true }),
				options,
			).accent,
		).toBeNull();
	});

	it('sets the language and its direction, unless the direction is forced', () => {
		const locale = (language: string, direction: 'auto' | 'ltr' | 'rtl') => ({
			...DEFAULT_SETTINGS,
			locale: { language, direction },
		});
		expect(resolveAppearance(locale('system', 'auto'), os({}), options)).toMatchObject({
			lang: 'en-CA',
			dir: 'ltr',
		});
		expect(resolveAppearance(locale('fr-CA', 'auto'), os({}), options).lang).toBe('fr-CA');
		expect(resolveAppearance(locale('fr-CA', 'rtl'), os({}), options).dir).toBe('rtl');
		expect(directionFor('ar-EG')).toBe('rtl');
		expect(directionFor('he')).toBe('rtl');
		expect(directionFor('en-CA')).toBe('ltr');
	});
});

describe('applyAppearance', () => {
	it('writes attributes and custom properties, and removes the accent override when it ends', () => {
		const root = document.createElement('html');
		const dark = resolveAppearance(
			withAppearance({ accent: { kind: 'custom', hex: '#0f766e' }, density: 'compact' }),
			os({ scheme: 'dark' }),
			options,
		);
		applyAppearance(root, dark);
		expect(root.dataset).toMatchObject({
			theme: 'dark',
			contrast: 'normal',
			density: 'compact',
			touch: 'false',
			motion: 'full',
			transparency: 'off',
		});
		expect(root.style.getPropertyValue('--wp-accent')).toMatch(/^#/);
		expect(root.style.getPropertyValue('--wp-accent-contrast')).toBe(
			root.style.getPropertyValue('--wp-accent-fg'),
		);
		expect(root.style.getPropertyValue('--wp-text-scale')).toBe('1');
		applyAppearance(root, resolveAppearance(DEFAULT_SETTINGS, os({ scheme: null }), options));
		expect(root.dataset.theme).toBeUndefined();
		expect(root.style.getPropertyValue('--wp-accent')).toBe('');
		expect(root.lang).toBe('en-CA');
		expect(root.dir).toBe('ltr');
	});
});
