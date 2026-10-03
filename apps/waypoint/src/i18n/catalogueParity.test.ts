// Tests the parity helper, and that every catalogue agrees with English on keys and tokens
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { catalogueParity, INCOMPLETE_LOCALES, parityFailures } from './catalogueParity';
import { loadCatalogue } from './loadCatalogue';
import { LOCALES, PSEUDO_LOCALES, type Locale } from './locales';
import { enMessages } from './messages';

describe('catalogueParity', () => {
	it('lists the keys a catalogue lacks and the ones it adds', () => {
		const parity = catalogueParity({
			'tabs.count.one': '{count} onglet',
			'nope.nope': 'x',
		} as never);
		expect(parity.missing).toContain('tabs.count.other');
		expect(parity.missing).not.toContain('tabs.count.one');
		expect(parity.extra).toEqual(['nope.nope']);
	});

	it('flags a message whose {tokens} differ from English', () => {
		const parity = catalogueParity({
			'tabs.count.one': '{nombre} onglet',
			'tabs.count.other': '{count} onglets',
		});
		expect(parity.tokenMismatch).toEqual(['tabs.count.one']);
	});

	it('fails on a missing key for a complete locale and not for one listed as incomplete', () => {
		const partial = { 'tabs.count.other': '{count} onglets' };
		expect(parityFailures('en-XA', partial).some((f) => f.includes('lacks'))).toBe(true);
		expect(parityFailures('fr-CA', partial).some((f) => f.includes('lacks'))).toBe(true);
		expect(parityFailures('fr-CA', { 'x.y': 'z' } as never).some((f) => f.includes('x.y'))).toBe(
			true,
		);
	});
});

describe('every catalogue against English', () => {
	const incomplete: readonly Locale[] = INCOMPLETE_LOCALES;
	it('lists no incomplete locale: fr-CA ships complete', () => {
		expect(incomplete).toEqual([]);
	});

	for (const locale of [...LOCALES, ...PSEUDO_LOCALES]) {
		it(`${locale} has no extra key and no token mismatch${incomplete.includes(locale) ? '' : ', and no missing key'}`, async () => {
			const catalogue = locale === 'en-CA' ? enMessages : await loadCatalogue(locale);
			expect(parityFailures(locale, catalogue as never)).toEqual([]);
		});
	}
});
