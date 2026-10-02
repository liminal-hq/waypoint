// The locales Waypoint has catalogues for, and how a setting and the OS language choose among them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The languages that ship, as BCP 47 tags. `en-CA` is the source catalogue and the fallback. */
export const LOCALES = ['en-CA', 'fr-CA'] as const;

/**
 * Developer-only pseudo-locales: `en-XA` is English with accented letters and about a third more
 * text, `ar-XB` is the same in a right-to-left wrapper. They prove the loader, text expansion and
 * mirrored layout, and are offered, loaded and accepted only in a developer build (A68, D119).
 */
export const PSEUDO_LOCALES = ['en-XA', 'ar-XB'] as const;

export type ShippedLocale = (typeof LOCALES)[number];
export type PseudoLocale = (typeof PSEUDO_LOCALES)[number];
export type Locale = ShippedLocale | PseudoLocale;

/** The catalogue every missing message falls back to. */
export const FALLBACK_LOCALE: ShippedLocale = 'en-CA';

export function isPseudoLocale(tag: string): tag is PseudoLocale {
	return (PSEUDO_LOCALES as readonly string[]).includes(tag);
}

/** Whether `tag` is a locale with a catalogue, counting the pseudo-locales only for a developer build. */
export function isAvailableLocale(tag: string, developer: boolean): tag is Locale {
	return (LOCALES as readonly string[]).includes(tag) || (developer && isPseudoLocale(tag));
}

/** What `Intl` should be given for a locale: a pseudo-locale borrows a real one that shows what it proves. */
function intlTagFor(locale: Locale): string {
	switch (locale) {
		case 'en-XA':
			return 'en-CA';
		case 'ar-XB':
			// Arabic numerals and separators, so a right-to-left window shows them as they come out.
			return 'ar';
		default:
			return locale;
	}
}

/** The plural rules that fit a locale's catalogue: a pseudo-locale's text is still English. */
export function pluralTagFor(locale: Locale): string {
	return isPseudoLocale(locale) ? 'en' : locale;
}

/** The nearest shipped catalogue for an OS language such as `fr-FR` or `de-DE`. */
export function nearestLocale(systemLanguage: string): ShippedLocale {
	const primary = systemLanguage.split(/[-_]/)[0]?.toLowerCase();
	return primary === 'fr' ? 'fr-CA' : FALLBACK_LOCALE;
}

/** What the locale setting and the OS language settle on. */
export interface ResolvedLocale {
	/** The catalogue messages come from. */
	locale: Locale;
	/** The tag dates, numbers and sizes are formatted for: the OS locale itself for "System default". */
	formatTag: string;
}

/**
 * The locale to use. `setting` is `system` or a locale tag; a tag that is not available here (a
 * pseudo-locale in a release build, or a language removed since) is treated as `system`.
 */
export function resolveLocale(
	setting: string,
	systemLanguage: string,
	developer: boolean,
): ResolvedLocale {
	if (isAvailableLocale(setting, developer)) {
		return { locale: setting, formatTag: intlTagFor(setting) };
	}
	return { locale: nearestLocale(systemLanguage), formatTag: systemLanguage };
}
