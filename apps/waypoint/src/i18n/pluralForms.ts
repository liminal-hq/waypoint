// Which plural forms a locale needs and how a plural group of messages expands to them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** The CLDR plural categories, in the order the rules list them. */
export type PluralCategory = Intl.LDMLPluralRule;

export const PLURAL_CATEGORY_ORDER: readonly PluralCategory[] = [
	'zero',
	'one',
	'two',
	'few',
	'many',
	'other',
];

/**
 * The categories a language's number rules distinguish, from `Intl.PluralRules`, in CLDR order
 * whatever order the engine lists them in (Node and Bun differ). English is `one` and `other`;
 * French is `one`, `many` and `other` (`many` is the "1 million de fichiers" form); Polish adds
 * `few`; Arabic has all six.
 */
export function pluralCategories(tag: string): PluralCategory[] {
	const have = new Set<string>(new Intl.PluralRules(tag).resolvedOptions().pluralCategories);
	return PLURAL_CATEGORY_ORDER.filter((category) => have.has(category));
}

function isCategory(word: string): word is PluralCategory {
	return (PLURAL_CATEGORY_ORDER as readonly string[]).includes(word);
}

/**
 * The plural groups in an English key list: `base` is one when both `base.one` and `base.other`
 * exist. A lone `.other` is a plain message that happens to say "other" (`openWith.other`).
 */
export function pluralBases(englishKeys: readonly string[]): Set<string> {
	const all = new Set(englishKeys);
	return new Set(
		englishKeys
			.filter((key) => key.endsWith('.other'))
			.map((key) => key.slice(0, -'.other'.length))
			.filter((base) => all.has(`${base}.one`)),
	);
}

/** The group a key is a form of (`tabs.count.one` is in `tabs.count`), or `undefined` for a plain message. */
export function pluralBaseOf(key: string, bases: ReadonlySet<string>): string | undefined {
	const dot = key.lastIndexOf('.');
	if (dot < 0 || !isCategory(key.slice(dot + 1))) return undefined;
	const base = key.slice(0, dot);
	return bases.has(base) ? base : undefined;
}

/**
 * The keys a catalogue in the language `tag` is expected to hold, in English source order: every
 * plain message, and for each plural group one message per category the language needs (a group
 * stands where its first English form is).
 */
export function expectedKeys(englishKeys: readonly string[], tag: string): string[] {
	const bases = pluralBases(englishKeys);
	const categories = pluralCategories(tag);
	const seen = new Set<string>();
	const keys: string[] = [];
	for (const key of englishKeys) {
		const base = pluralBaseOf(key, bases);
		if (base === undefined) {
			keys.push(key);
		} else if (!seen.has(base)) {
			seen.add(base);
			for (const category of categories) keys.push(`${base}.${category}`);
		}
	}
	return keys;
}

/**
 * The English key a message is translated from: itself, or for a plural form English does not have
 * (Polish `few`, French `many`) the group's `.other`. `undefined` for a key that is not expected.
 */
export function sourceKeyFor(key: string, englishKeys: ReadonlySet<string>): string | undefined {
	if (englishKeys.has(key)) return key;
	const dot = key.lastIndexOf('.');
	if (dot < 0 || !isCategory(key.slice(dot + 1))) return undefined;
	const base = key.slice(0, dot);
	return englishKeys.has(`${base}.one`) && englishKeys.has(`${base}.other`)
		? `${base}.other`
		: undefined;
}
