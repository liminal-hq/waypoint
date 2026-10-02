// Tests how a language setting and the OS language settle on a catalogue and the Intl tags
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { isAvailableLocale, nearestLocale, pluralTagFor, resolveLocale } from './locales';

describe('nearestLocale', () => {
	it('maps an OS language to the nearest shipped catalogue, English when there is none', () => {
		expect(nearestLocale('fr-FR')).toBe('fr-CA');
		expect(nearestLocale('fr')).toBe('fr-CA');
		expect(nearestLocale('en-GB')).toBe('en-CA');
		expect(nearestLocale('de-DE')).toBe('en-CA');
		expect(nearestLocale('ar-SA')).toBe('en-CA');
	});
});

describe('resolveLocale', () => {
	it('follows the OS for "system", formatting with the OS locale itself', () => {
		expect(resolveLocale('system', 'fr-FR', false)).toEqual({
			locale: 'fr-CA',
			formatTag: 'fr-FR',
		});
		expect(resolveLocale('system', 'de-DE', false)).toEqual({
			locale: 'en-CA',
			formatTag: 'de-DE',
		});
	});

	it('uses a chosen language for both the messages and the formats, whatever the OS says', () => {
		expect(resolveLocale('en-CA', 'fr-FR', false)).toEqual({ locale: 'en-CA', formatTag: 'en-CA' });
		expect(resolveLocale('fr-CA', 'en-US', false)).toEqual({ locale: 'fr-CA', formatTag: 'fr-CA' });
	});

	it('offers the pseudo-locales only to a developer build, borrowing a real locale for Intl', () => {
		expect(resolveLocale('en-XA', 'en-US', true)).toEqual({ locale: 'en-XA', formatTag: 'en-CA' });
		expect(resolveLocale('ar-XB', 'en-US', true)).toEqual({ locale: 'ar-XB', formatTag: 'ar' });
		expect(resolveLocale('ar-XB', 'fr-FR', false)).toEqual({ locale: 'fr-CA', formatTag: 'fr-FR' });
		expect(isAvailableLocale('en-XA', false)).toBe(false);
		expect(isAvailableLocale('en-XA', true)).toBe(true);
	});

	it('treats an unknown language as "system"', () => {
		expect(resolveLocale('xx', 'fr-FR', true)).toEqual({ locale: 'fr-CA', formatTag: 'fr-FR' });
	});
});

describe('pluralTagFor', () => {
	it('gives a pseudo-locale English rules, since its text is English', () => {
		expect(pluralTagFor('ar-XB')).toBe('en');
		expect(pluralTagFor('en-XA')).toBe('en');
		expect(pluralTagFor('fr-CA')).toBe('fr-CA');
	});
});
