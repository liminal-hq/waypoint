// Loads one catalogue per locale on demand and makes it the one in force
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { setActiveLocale, type Catalogue } from './active';
import { FALLBACK_LOCALE, resolveLocale, type Locale, type ResolvedLocale } from './locales';

/** The loader a release build has for a pseudo-locale: none. */
function developerOnly(locale: Locale): () => Promise<{ default: Catalogue }> {
	return () => Promise.reject(new Error(`${locale} is only available in a developer build`));
}

/**
 * One dynamic import per locale, so a window carries the English source and only the catalogue it
 * uses. English is always in the bundle: it is the fallback for any key another locale lacks.
 */
const MODULES: Record<Locale, () => Promise<{ default: Catalogue }>> = {
	'en-CA': async () => ({ default: {} }),
	'fr-CA': () => import('./catalogues/fr-CA'),
	// The pseudo-locales are developer-only: a release build replaces `import.meta.env.DEV` with
	// `false`, which drops these imports and their modules from the bundle altogether.
	'en-XA': import.meta.env.DEV ? () => import('./catalogues/en-XA') : developerOnly('en-XA'),
	'ar-XB': import.meta.env.DEV ? () => import('./catalogues/ar-XB') : developerOnly('ar-XB'),
};

const loaded = new Map<Locale, Promise<Catalogue>>();

/** The catalogue for `locale`, loaded once. A load that fails is forgotten so the next try can retry. */
export function loadCatalogue(locale: Locale): Promise<Catalogue> {
	let pending = loaded.get(locale);
	if (!pending) {
		pending = MODULES[locale]().then(
			(module) => module.default,
			(error: unknown) => {
				loaded.delete(locale);
				throw error;
			},
		);
		loaded.set(locale, pending);
	}
	return pending;
}

/** Forgets what has been loaded; for tests. */
export function forgetCatalogues(): void {
	loaded.clear();
}

/** What `activateLocale` is given. */
export interface LocaleChoice {
	/** The `locale.language` setting: `system` or a locale tag. */
	setting: string;
	/** The OS language, such as `fr-FR`. */
	systemLanguage: string;
	/** Whether this is a developer build, where the pseudo-locales exist. */
	developer: boolean;
}

/**
 * Loads the catalogue the setting and the OS language settle on, then makes it the one in force,
 * all at once: a caller that waits for this renders in the new language from its first frame.
 * When the catalogue cannot be loaded the window stays in English, which is what `t` falls back to.
 */
export async function activateLocale(choice: LocaleChoice): Promise<ResolvedLocale> {
	const resolved = resolveLocale(choice.setting, choice.systemLanguage, choice.developer);
	try {
		const catalogue = await loadCatalogue(resolved.locale);
		setActiveLocale(resolved.locale, catalogue, resolved.formatTag);
		return resolved;
	} catch (error) {
		console.warn(`The ${resolved.locale} catalogue could not be loaded; using English.`, error);
		setActiveLocale(FALLBACK_LOCALE, {}, resolved.formatTag);
		return { locale: FALLBACK_LOCALE, formatTag: resolved.formatTag };
	}
}
