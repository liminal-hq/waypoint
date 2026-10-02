// Compares a catalogue's keys and tokens with English: the check slice 22 turns on for `fr-CA`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Catalogue } from './active';
import { enMessages } from './messages';
import type { Locale } from './locales';

/**
 * Locales whose catalogue is known to be incomplete, so a missing key is not yet a failure. Empty
 * the entry for `fr-CA` when the full translation lands and the parity test starts failing on a gap.
 */
export const INCOMPLETE_LOCALES: readonly Locale[] = ['fr-CA'];

export interface Parity {
	/** Keys English has and the catalogue lacks. */
	missing: string[];
	/** Keys the catalogue has and English does not. */
	extra: string[];
	/** Keys whose `{name}` tokens differ from English's, a bug in a translation. */
	tokenMismatch: string[];
}

function tokens(message: string): string {
	return [...message.matchAll(/\{(\w+)\}/g)]
		.map((match) => match[1])
		.sort()
		.join(',');
}

/** How `catalogue` differs from the English source. */
export function catalogueParity(catalogue: Catalogue): Parity {
	const english = enMessages as Record<string, string>;
	const own = catalogue as Record<string, string>;
	return {
		missing: Object.keys(english).filter((key) => !(key in own)),
		extra: Object.keys(own).filter((key) => !(key in english)),
		tokenMismatch: Object.keys(own).filter(
			(key) => key in english && tokens(own[key]!) !== tokens(english[key]!),
		),
	};
}

/**
 * The problems that fail the parity test for `locale`: always an extra key or a token mismatch, and
 * a missing key too unless the locale is listed as incomplete.
 */
export function parityFailures(locale: Locale, catalogue: Catalogue): string[] {
	const parity = catalogueParity(catalogue);
	const failures = [
		...parity.extra.map((key) => `${locale} has ${key}, which English does not`),
		...parity.tokenMismatch.map((key) => `${locale} ${key} has different {tokens} from English`),
	];
	if (!INCOMPLETE_LOCALES.includes(locale)) {
		failures.push(...parity.missing.map((key) => `${locale} lacks ${key}`));
	}
	return failures;
}
