// Compares a catalogue's keys and tokens with English: the check slice 22 turns on for `fr-CA`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Catalogue } from './active';
import { enMessages } from './messages';
import { FALLBACK_LOCALE, pluralTagFor, type Locale } from './locales';
import { expectedKeys, sourceKeyFor } from './pluralForms';

/**
 * Locales whose catalogue is known to be incomplete, so a missing key is not yet a failure. Empty
 * now that `fr-CA` is complete; list a language here while it is being written, and take it out
 * when it reaches 100% so a gap fails the parity test.
 */
export const INCOMPLETE_LOCALES: readonly Locale[] = [];

export interface Parity {
	/** Keys English has and the catalogue lacks. */
	missing: string[];
	/** Keys the catalogue has and English does not. */
	extra: string[];
	/** Keys whose `{name}` tokens differ from English's, a bug in a translation. */
	tokenMismatch: string[];
}

/** A message's `{name}` tokens, sorted and joined, to compare two messages. */
export function tokens(message: string): string {
	return [...message.matchAll(/\{(\w+)\}/g)]
		.map((match) => match[1])
		.sort()
		.join(',');
}

/**
 * How `catalogue` differs from the English source. The keys a locale is expected to hold are the
 * English ones with each plural group spelled out in the categories `Intl.PluralRules` lists for
 * the locale (Polish needs `few` and `many`, French `one`, `many` and `other`).
 */
export function catalogueParity(catalogue: Catalogue, locale: Locale = FALLBACK_LOCALE): Parity {
	const english = enMessages as Record<string, string>;
	const englishKeys = Object.keys(english);
	const own = catalogue as Record<string, string>;
	const expected = expectedKeys(englishKeys, pluralTagFor(locale));
	const expectedSet = new Set(expected);
	const englishSet = new Set(englishKeys);
	return {
		missing: expected.filter((key) => !(key in own)),
		extra: Object.keys(own).filter((key) => !expectedSet.has(key)),
		tokenMismatch: Object.keys(own).filter((key) => {
			const source = expectedSet.has(key) ? sourceKeyFor(key, englishSet) : undefined;
			return source !== undefined && tokens(own[key]!) !== tokens(english[source]!);
		}),
	};
}

/**
 * The problems that fail the parity test for `locale`: always an extra key or a token mismatch, and
 * a missing key too unless the locale is listed as incomplete.
 */
export function parityFailures(locale: Locale, catalogue: Catalogue): string[] {
	const parity = catalogueParity(catalogue, locale);
	const failures = [
		...parity.extra.map((key) => `${locale} has ${key}, which English does not`),
		...parity.tokenMismatch.map((key) => `${locale} ${key} has different {tokens} from English`),
	];
	if (!INCOMPLETE_LOCALES.includes(locale)) {
		failures.push(...parity.missing.map((key) => `${locale} lacks ${key}`));
	}
	return failures;
}
