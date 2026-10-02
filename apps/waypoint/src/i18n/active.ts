// The locale in force: its catalogue, the `Intl` tags and a version that changes when it does
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSyncExternalStore } from 'react';
import { FALLBACK_LOCALE, pluralTagFor, type Locale } from './locales';
import type { MessageId } from './messages';

/** A catalogue: some or all of the messages, in one locale. A missing one falls back to English. */
export type Catalogue = Partial<Record<MessageId, string>>;

interface ActiveLocale {
	locale: Locale;
	messages: Catalogue;
	formatTag: string | undefined;
	version: number;
}

let active: ActiveLocale = {
	locale: FALLBACK_LOCALE,
	messages: {},
	formatTag: undefined,
	version: 0,
};
const listeners = new Set<() => void>();

/** The locale messages are in now. */
export function activeLocale(): Locale {
	return active.locale;
}

/** The messages `t` reads before it falls back to English. */
export function activeMessages(): Catalogue {
	return active.messages;
}

/** The tag to hand `Intl.NumberFormat`, `Intl.DateTimeFormat` and the rest; `undefined` is the runtime's own. */
export function formatLocale(): string | undefined {
	return active.formatTag;
}

/** The tag to hand `Intl.PluralRules`, for the language the catalogue is in. */
export function pluralLocale(): string {
	return pluralTagFor(active.locale);
}

/** Changes whenever `setActiveLocale` is called, for `useSyncExternalStore`. */
export function localeVersion(): number {
	return active.version;
}

/** Re-renders the component that calls it whenever the locale in force changes. */
export function useLocaleVersion(): number {
	return useSyncExternalStore(subscribeLocale, localeVersion);
}

export function subscribeLocale(listener: () => void): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}

/** Makes `messages` (already loaded) the catalogue in force, and tells whoever is listening. */
export function setActiveLocale(
	locale: Locale,
	messages: Catalogue,
	formatTag: string | undefined,
): void {
	active = { locale, messages, formatTag, version: active.version + 1 };
	for (const listener of [...listeners]) listener();
}

/** Puts the English catalogue back; for tests. */
export function resetActiveLocale(): void {
	setActiveLocale(FALLBACK_LOCALE, {}, undefined);
}
