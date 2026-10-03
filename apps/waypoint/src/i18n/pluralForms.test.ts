// Tests plural categories per language, `tn` choosing among them, and parity expecting exactly them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { resetActiveLocale, setActiveLocale, type Catalogue } from './active';
import { catalogueParity, parityFailures } from './catalogueParity';
import { enMessages, tn } from './messages';
import {
	expectedKeys,
	pluralBases,
	pluralBaseOf,
	pluralCategories,
	sourceKeyFor,
} from './pluralForms';

afterEach(resetActiveLocale);

/** A synthetic catalogue for `tabs.count`, the way a translator in a language with these forms writes it. */
function withCatalogue(messages: Catalogue): void {
	setActiveLocale('fr-CA', messages, undefined);
}

describe('pluralCategories', () => {
	it('lists the categories each language distinguishes, in CLDR order', () => {
		expect(pluralCategories('en')).toEqual(['one', 'other']);
		expect(pluralCategories('fr-CA')).toEqual(['one', 'many', 'other']);
		expect(pluralCategories('pl')).toEqual(['one', 'few', 'many', 'other']);
		expect(pluralCategories('ar')).toEqual(['zero', 'one', 'two', 'few', 'many', 'other']);
		expect(pluralCategories('ja')).toEqual(['other']);
	});
});

describe('plural groups', () => {
	const keys = Object.keys(enMessages);

	it('are the keys with both a .one and an .other, and not a lone .other', () => {
		const bases = pluralBases(keys);
		expect(bases.has('tabs.count')).toBe(true);
		expect(bases.has('openWith')).toBe(false);
		expect(bases.has('browse.group')).toBe(false);
		expect(pluralBaseOf('tabs.count.few', bases)).toBe('tabs.count');
		expect(pluralBaseOf('openWith.other', bases)).toBeUndefined();
	});

	it('expand to the categories of the language, where the English group stands', () => {
		const en = expectedKeys(keys, 'en');
		expect(en).toEqual(keys);
		const pl = expectedKeys(keys, 'pl');
		const at = pl.indexOf('tabs.count.one');
		expect(pl.slice(at, at + 4)).toEqual([
			'tabs.count.one',
			'tabs.count.few',
			'tabs.count.many',
			'tabs.count.other',
		]);
		expect(pl.filter((key) => key.startsWith('tabs.count.'))).toHaveLength(4);
	});

	it('translate from the English other when English has no such form', () => {
		const set = new Set(keys);
		expect(sourceKeyFor('tabs.count.few', set)).toBe('tabs.count.other');
		expect(sourceKeyFor('tabs.count.one', set)).toBe('tabs.count.one');
		expect(sourceKeyFor('openWith.few', set)).toBeUndefined();
	});
});

describe('tn with every category', () => {
	it('uses one and other in English', () => {
		expect(tn('tabs.count', 1, 'en')).toBe('1 tab');
		expect(tn('tabs.count', 2, 'en')).toBe('2 tabs');
	});

	it('uses one, many and other in French, falling back to other for a catalogue without many', () => {
		withCatalogue({
			'tabs.count.one': '{count} onglet',
			'tabs.count.many': '{count} d’onglets',
			'tabs.count.other': '{count} onglets',
		});
		expect(tn('tabs.count', 0, 'fr-CA')).toBe('0 onglet');
		expect(tn('tabs.count', 2, 'fr-CA')).toBe('2 onglets');
		expect(tn('tabs.count', 1_000_000, 'fr-CA').replace(/\s/g, ' ')).toBe('1 000 000 d’onglets');
		withCatalogue({ 'tabs.count.one': '{count} onglet', 'tabs.count.other': '{count} onglets' });
		expect(tn('tabs.count', 1_000_000, 'fr-CA').replace(/\s/g, ' ')).toBe('1 000 000 onglets');
	});

	it('uses few and many in Polish', () => {
		withCatalogue({
			'tabs.count.one': '{count} karta',
			'tabs.count.few': '{count} karty',
			'tabs.count.many': '{count} kart',
			'tabs.count.other': '{count} karty',
		});
		expect(tn('tabs.count', 1, 'pl')).toBe('1 karta');
		expect(tn('tabs.count', 3, 'pl')).toBe('3 karty');
		expect(tn('tabs.count', 5, 'pl')).toBe('5 kart');
		expect(tn('tabs.count', 22, 'pl')).toBe('22 karty');
		expect(tn('tabs.count', 12, 'pl')).toBe('12 kart');
	});

	it('uses all six in Arabic', () => {
		const forms: Record<string, string> = {
			zero: 'صفر',
			one: 'واحد',
			two: 'اثنان',
			few: 'قليل',
			many: 'كثير',
			other: 'آخر',
		};
		withCatalogue(
			Object.fromEntries(Object.entries(forms).map(([form, text]) => [`tabs.count.${form}`, text])),
		);
		const chosen = [0, 1, 2, 3, 11, 100].map((count) => tn('tabs.count', count, 'ar'));
		expect(chosen).toEqual([forms.zero, forms.one, forms.two, forms.few, forms.many, forms.other]);
	});

	it('falls back to other when neither the catalogue nor English has the form', () => {
		withCatalogue({ 'tabs.count.other': '{count} karty' });
		expect(tn('tabs.count', 5, 'pl')).toBe('5 karty');
	});
});

describe('parity against the categories of a locale', () => {
	it('expects exactly one and other in English and flags a form English does not use', () => {
		expect(
			catalogueParity({ 'tabs.count.one': 'a', 'tabs.count.other': 'b' }).missing,
		).not.toContain('tabs.count.other');
		expect(catalogueParity({ 'tabs.count.few': 'x' }).extra).toEqual(['tabs.count.few']);
	});

	it('expects the extra forms of a language, and no others', () => {
		// The pseudo-locales read as English, so only the shipped fr-CA shows the French forms.
		const french = catalogueParity({ 'tabs.count.many': 'x', 'tabs.count.few': 'y' }, 'fr-CA');
		expect(french.extra).toEqual(['tabs.count.few']);
		expect(french.missing).toContain('tabs.count.one');
		expect(french.missing).not.toContain('tabs.count.many');
		expect(
			parityFailures('fr-CA', { 'tabs.count.few': 'y' }).filter((f) => !f.includes(' lacks ')),
		).toEqual(['fr-CA has tabs.count.few, which English does not']);
	});

	it('compares a form English lacks with the English other for its placeholders', () => {
		const parity = catalogueParity({ 'tabs.count.many': '{nombre} onglets' }, 'fr-CA');
		expect(parity.tokenMismatch).toEqual(['tabs.count.many']);
	});
});
