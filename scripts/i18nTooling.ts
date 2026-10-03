// The translator tooling behind `bun run i18n:*`: export a catalogue as JSON, import it back, and report what is stale
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { format, resolveConfig } from 'prettier';
import {
	expectedKeys,
	pluralBaseOf,
	pluralBases,
	sourceKeyFor,
} from '../apps/waypoint/src/i18n/pluralForms';

/*
 * Two files per language carry the same messages: the TypeScript catalogue the app imports
 * (`catalogues/<locale>.ts`) and the JSON a translator edits (`translations/<locale>.json`). The
 * JSON adds a note per key and the hash of the English text each translation was made from; the
 * catalogue keeps that hash in a `// source: <hash>` comment above the key so that exporting and
 * importing again changes nothing. English is the one source: `messages.ts`.
 */

/** One `'key': 'message'` of an object literal, with the comment lines written above it. */
export interface SourceEntry {
	key: string;
	message: string;
	/** The text of each `//` comment since the previous entry, without the slashes. */
	comments: string[];
}

/** One message of a translation as it is written to a catalogue. */
export interface CatalogueEntry {
	key: string;
	message: string;
	/** The hash of the English text this was translated from. */
	source: string;
}

/** One entry of a `translations/<locale>.json` file. */
export interface TranslationEntry {
	message: string;
	description: string;
	/** Present in a translation file, absent from the English one. */
	source?: string;
}

export const TRANSLATOR_PREFIX = 'translator:';
export const SOURCE_PREFIX = 'source:';

/** What each placeholder usually stands for, shown to translators in a key's note. */
export const TOKEN_MEANINGS: Record<string, string> = {
	count: 'a number of items (the plural form is chosen from it)',
	name: 'the name of a file, folder, tab or other item',
	reason: 'why something failed, in the system’s own words',
	message: 'a message from the system, which may be in English',
	features: 'a list of what works',
	title: 'the title of a tab',
	titles: 'the titles of tabs, as a list',
	tabs: 'a number of tabs, already worded (“3 tabs”)',
	total: 'a total, a number or a size',
	shown: 'how many are shown',
	done: 'how many are finished',
	read: 'how much has been read, as a size',
	label: 'the name of an action or item, such as what Undo would undo',
	what: 'what is being dragged, such as “3 items”',
	target: 'where something is being dropped or sent',
	destination: 'the folder something is being copied or moved to',
	location: 'a folder or path',
	path: 'a folder or file path',
	folder: 'the name of a folder',
	folders: 'a number of folders, already worded',
	files: 'a number of files, already worded',
	item: 'the name of one item',
	position: 'a place in a list or row, a number',
	percent: 'a percentage, a number without the % sign',
	size: 'a file or disk size with its unit',
	bytes: 'a number of bytes',
	needed: 'a size of space needed',
	free: 'a size of free space',
	used: 'a size of used space',
	action: 'a verb such as “eject” or “copy”',
	from: 'the original name or value',
	to: 'the new name or value',
	time: 'a time or a length of time',
	speed: 'a transfer speed with its unit',
	number: 'a number, such as a rule’s place in a list',
	window: 'the name of a window',
	limit: 'a maximum number',
	min: 'the smallest allowed value',
	max: 'the largest allowed value',
	colour: 'the name of a colour',
	second: 'the second of two things',
	first: 'the first of two things',
	other: 'another name, or a count of the rest',
	line: 'a line number',
	keys: 'a keyboard shortcut',
	algorithm: 'the name of a checksum algorithm, such as SHA-256',
	digest: 'a checksum, a string of letters and digits',
	open: 'a number of items still open',
	covered: 'a number of items covered',
	n: 'a number',
	h: 'hours, a number',
	m: 'minutes, a number',
	input: 'text the person typed',
	id: 'an identifier',
	group: 'the name of a group',
	by: 'what is using something',
	year: 'a year',
	width: 'a width in pixels',
	height: 'a height in pixels',
	volume: 'the name of a drive or volume',
	text: 'text to show as it is',
	summary: 'a short summary sentence',
	row: 'the name of a list row',
	opened: 'how many were opened',
	layout: 'a word for how panes are arranged',
	kept: 'the name of the tab that stays',
	closed: 'the name of the tab that closed',
	form: 'a plural form name',
	extension: 'a file extension, such as .txt',
	edge: 'a side of the window: left, right, top or bottom',
	choice: 'the name of a choice',
	character: 'a character that is not allowed',
	app: 'the name of an application',
	answered: 'how many have been answered',
	'1': 'a literal “${1}” group reference in a regular expression; keep it exactly',
};

/** The language names used in a generated catalogue's header; any other locale shows its tag. */
export const LANGUAGE_NAMES: Record<string, string> = {
	'fr-CA': 'Canadian French',
	'en-CA': 'Canadian English',
};

/** A short, stable fingerprint of an English message: eight hex digits of its SHA-256. */
export function hashMessage(message: string): string {
	return createHash('sha256').update(message).digest('hex').slice(0, 8);
}

/** The `{name}` tokens in a message, one entry per occurrence, sorted. */
export function placeholders(message: string): string[] {
	return [...message.matchAll(/\{(\w+)\}/g)].map((match) => match[1]!).sort();
}

const sameItems = (a: string[], b: string[]): boolean => a.join(',') === b.join(',');

/* ---------------------------------------------------------------- reading a catalogue's source */

class SourceError extends Error {}

function lineOf(text: string, index: number): number {
	return text.slice(0, index).split('\n').length;
}

function readString(text: string, start: number): { value: string; end: number } {
	const quote = text[start]!;
	let value = '';
	let i = start + 1;
	for (; i < text.length; i += 1) {
		const ch = text[i]!;
		if (ch === quote) return { value, end: i + 1 };
		if (quote === '`' && ch === '$' && text[i + 1] === '{') {
			throw new SourceError(`line ${lineOf(text, i)}: a template with \${} is not supported`);
		}
		if (ch === '\n' && quote !== '`') break;
		if (ch !== '\\') {
			value += ch;
			continue;
		}
		const next = text[i + 1];
		i += 1;
		switch (next) {
			case 'n':
				value += '\n';
				break;
			case 't':
				value += '\t';
				break;
			case 'r':
				value += '\r';
				break;
			case 'b':
				value += '\b';
				break;
			case 'f':
				value += '\f';
				break;
			case 'v':
				value += '\v';
				break;
			case '0':
				value += '\0';
				break;
			case '\n':
				break;
			case 'x':
				value += String.fromCharCode(parseInt(text.slice(i + 1, i + 3), 16));
				i += 2;
				break;
			case 'u': {
				if (text[i + 1] === '{') {
					const close = text.indexOf('}', i);
					value += String.fromCodePoint(parseInt(text.slice(i + 2, close), 16));
					i = close;
				} else {
					value += String.fromCharCode(parseInt(text.slice(i + 1, i + 5), 16));
					i += 4;
				}
				break;
			}
			default:
				value += next ?? '';
		}
	}
	throw new SourceError(`line ${lineOf(text, start)}: a string is not closed`);
}

/**
 * The entries of the first `const name = { 'key': 'message', … }` object literal in a module, with
 * the `//` comments written above each. A message must be one string literal; anything else (a
 * concatenation, a computed key) is an error rather than a guess.
 */
export function parseObjectSource(text: string): SourceEntry[] {
	const open = /^(?:export\s+)?const\s+\w+(?:\s*:\s*[\w<>, ]+)?\s*=\s*\{/m.exec(text);
	if (!open) throw new SourceError('no `const name = {` object literal found');
	const entries: SourceEntry[] = [];
	let comments: string[] = [];
	let i = open.index + open[0].length;

	const skip = (collect: boolean): void => {
		for (;;) {
			while (i < text.length && /\s/.test(text[i]!)) i += 1;
			if (text.startsWith('//', i)) {
				const end = text.indexOf('\n', i);
				const stop = end < 0 ? text.length : end;
				if (collect) comments.push(text.slice(i + 2, stop).trim());
				i = stop;
			} else if (text.startsWith('/*', i)) {
				const end = text.indexOf('*/', i + 2);
				if (end < 0) throw new SourceError(`line ${lineOf(text, i)}: a comment is not closed`);
				i = end + 2;
			} else {
				return;
			}
		}
	};
	const expect = (char: string): void => {
		if (text[i] !== char) {
			throw new SourceError(`line ${lineOf(text, i)}: expected \`${char}\``);
		}
		i += 1;
	};

	for (;;) {
		skip(true);
		if (text[i] === '}') return entries;
		if (i >= text.length) throw new SourceError('the object literal is not closed');
		if (!/['"`]/.test(text[i]!)) {
			throw new SourceError(`line ${lineOf(text, i)}: a key must be a quoted string`);
		}
		const key = readString(text, i);
		i = key.end;
		skip(false);
		expect(':');
		skip(false);
		if (!/['"`]/.test(text[i] ?? '')) {
			throw new SourceError(
				`line ${lineOf(text, i)}: the message for ${key.value} must be a string`,
			);
		}
		const message = readString(text, i);
		i = message.end;
		skip(false);
		if (text[i] === '+') {
			throw new SourceError(
				`line ${lineOf(text, i)}: ${key.value} joins strings with +; write it as one string`,
			);
		}
		if (text[i] === ',') i += 1;
		else if (text[i] !== '}') throw new SourceError(`line ${lineOf(text, i)}: expected \`,\``);
		entries.push({ key: key.value, message: message.value, comments });
		comments = [];
	}
}

/**
 * The translator comment above a key: the comment lines from one starting `translator:` on, the
 * following plain `//` lines continuing it; joined into one line. `undefined` when there is none.
 */
export function translatorComment(comments: string[]): string | undefined {
	const start = comments.findIndex((line) => line.startsWith(TRANSLATOR_PREFIX));
	if (start < 0) return undefined;
	const lines = [comments[start]!.slice(TRANSLATOR_PREFIX.length).trim()];
	for (const line of comments.slice(start + 1)) {
		if (line.startsWith(SOURCE_PREFIX)) break;
		lines.push(line);
	}
	const text = lines.filter(Boolean).join(' ');
	return text || undefined;
}

/** The English hash a catalogue entry records in its `// source: <hash>` comment. */
export function recordedSource(comments: string[]): string | undefined {
	const line = comments.find((comment) => comment.startsWith(SOURCE_PREFIX));
	const hash = line?.slice(SOURCE_PREFIX.length).trim();
	return hash && /^[0-9a-f]{8}$/.test(hash) ? hash : undefined;
}

/* -------------------------------------------------------------------------------------- notes */

/** What a translator needs to know about English, to work out any key's note. */
export interface EnglishContext {
	entries: SourceEntry[];
	byKey: Map<string, SourceEntry>;
	keys: string[];
	keySet: Set<string>;
	bases: Set<string>;
}

export function englishContext(entries: SourceEntry[]): EnglishContext {
	const keys = entries.map((entry) => entry.key);
	return {
		entries,
		byKey: new Map(entries.map((entry) => [entry.key, entry])),
		keys,
		keySet: new Set(keys),
		bases: pluralBases(keys),
	};
}

/**
 * The note for `key`: the area (the key's first part), the placeholders with what they stand for,
 * which other keys are forms of the same plural, and the translator comment from the English source.
 * `tag` is the language the forms of a plural group are counted for.
 */
export function noteFor(key: string, english: EnglishContext, tag: string): string {
	const sourceKey = sourceKeyFor(key, english.keySet);
	if (sourceKey === undefined) return 'Not a message English has; remove it.';
	const parts = [`Area: ${key.split('.')[0]}.`];

	const tokens = [...new Set(placeholders(english.byKey.get(sourceKey)!.message))];
	if (tokens.length > 0) {
		const listed = tokens.map((token) => {
			const meaning = TOKEN_MEANINGS[token];
			return meaning ? `{${token}} is ${meaning}` : `{${token}}`;
		});
		parts.push(`Placeholders, kept exactly as written: ${listed.join('; ')}.`);
	}

	const base = pluralBaseOf(key, english.bases);
	const comments = [sourceKey];
	if (base !== undefined) {
		const forms = expectedKeys(english.keys, tag).filter(
			(form) => pluralBaseOf(form, english.bases) === base,
		);
		const others = forms.filter((form) => form !== key);
		parts.push(`Plural: one of a group; the other forms are ${others.join(', ')}.`);
		comments.push(...english.keys.filter((form) => pluralBaseOf(form, english.bases) === base));
	}
	for (const candidate of comments) {
		const comment = translatorComment(english.byKey.get(candidate)?.comments ?? []);
		if (comment) {
			parts.push(comment);
			break;
		}
	}
	return parts.join(' ');
}

/* -------------------------------------------------------------------------------------- export */

/** The `en-CA.json` content: every English message in source order with its note. */
export function exportEnglish(english: EnglishContext): Record<string, TranslationEntry> {
	const out: Record<string, TranslationEntry> = {};
	for (const entry of english.entries) {
		out[entry.key] = { message: entry.message, description: noteFor(entry.key, english, 'en') };
	}
	return out;
}

/**
 * A translation's JSON content: its translated messages in English source order (the keys a plural
 * group needs come where the group is), then any key English does not have, each with its note and
 * the hash of the English text it was translated from. An entry with no recorded hash is taken to
 * be current.
 */
export function exportTranslation(
	english: EnglishContext,
	translated: SourceEntry[],
	tag: string,
): Record<string, TranslationEntry> {
	const byKey = new Map(translated.map((entry) => [entry.key, entry]));
	const expected = expectedKeys(english.keys, tag);
	const expectedSet = new Set(expected);
	const extras = translated
		.map((entry) => entry.key)
		.filter((key) => !expectedSet.has(key))
		.sort();
	const out: Record<string, TranslationEntry> = {};
	for (const key of [...expected, ...extras]) {
		const entry = byKey.get(key);
		if (!entry) continue;
		const sourceKey = sourceKeyFor(key, english.keySet);
		const current = sourceKey ? hashMessage(english.byKey.get(sourceKey)!.message) : '';
		out[key] = {
			message: entry.message,
			description: noteFor(key, english, tag),
			source: recordedSource(entry.comments) ?? current,
		};
	}
	return out;
}

/** JSON as the tooling writes it: two-space indent and a trailing newline. */
export function stringifyTranslations(entries: Record<string, TranslationEntry>): string {
	return `${JSON.stringify(entries, null, 2)}\n`;
}

/* -------------------------------------------------------------------------------------- import */

export interface ImportResult {
	entries: CatalogueEntry[];
	errors: string[];
}

/**
 * Reads a translation file into catalogue entries in English source order. Refuses a key English
 * does not have (for this language) and a message whose `{placeholders}` differ from English's; an
 * empty message is an untranslated one and is left out. A missing `source` keeps the hash the
 * catalogue already records for an unchanged message and otherwise takes the current English text.
 */
export function importTranslation(
	json: string,
	english: EnglishContext,
	existing: SourceEntry[],
	tag: string,
	file = 'translation file',
): ImportResult {
	const errors: string[] = [];
	let parsed: unknown;
	try {
		parsed = JSON.parse(json);
	} catch (error) {
		return { entries: [], errors: [`${file} is not valid JSON: ${(error as Error).message}`] };
	}
	if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
		return { entries: [], errors: [`${file} must be one JSON object of keys to entries`] };
	}
	const expected = expectedKeys(english.keys, tag);
	const expectedSet = new Set(expected);
	const before = new Map(existing.map((entry) => [entry.key, entry]));
	const read = new Map<string, CatalogueEntry>();

	for (const [key, raw] of Object.entries(parsed)) {
		const value = typeof raw === 'string' ? { message: raw } : (raw as Record<string, unknown>);
		if (!expectedSet.has(key)) {
			errors.push(`${file}: ${key} is not a message English has for ${tag}`);
			continue;
		}
		if (typeof value?.message !== 'string') {
			errors.push(`${file}: ${key} has no "message" string`);
			continue;
		}
		if (value.message === '') continue;
		const sourceKey = sourceKeyFor(key, english.keySet)!;
		const englishMessage = english.byKey.get(sourceKey)!.message;
		if (!sameItems(placeholders(value.message), placeholders(englishMessage))) {
			errors.push(
				`${file}: ${key} has placeholders {${placeholders(value.message).join('}, {') || ''}} but English has {${placeholders(englishMessage).join('}, {')}}`,
			);
			continue;
		}
		const prior = before.get(key);
		const given = typeof value.source === 'string' ? value.source : undefined;
		const kept =
			prior && prior.message === value.message ? recordedSource(prior.comments) : undefined;
		read.set(key, {
			key,
			message: value.message,
			source: given && /^[0-9a-f]{8}$/.test(given) ? given : (kept ?? hashMessage(englishMessage)),
		});
	}
	return { entries: expected.flatMap((key) => read.get(key) ?? []), errors };
}

function quote(message: string): string {
	const escaped = message
		.replace(/\\/g, '\\\\')
		.replace(/'/g, "\\'")
		.replace(/\n/g, '\\n')
		.replace(/\r/g, '\\r')
		.replace(/\t/g, '\\t')
		// eslint-disable-next-line no-control-regex
		.replace(
			/[\u0000-\u001f\u007f]/g,
			(ch) => `\\u${ch.charCodeAt(0).toString(16).padStart(4, '0')}`,
		);
	return `'${escaped}'`;
}

/** The TypeScript catalogue for a locale, before Prettier lays it out. */
export function renderCatalogue(locale: string, entries: CatalogueEntry[]): string {
	const name = LANGUAGE_NAMES[locale] ?? locale;
	const body = entries
		.map(
			(entry) =>
				`\t// ${SOURCE_PREFIX} ${entry.source}\n\t${quote(entry.key)}: ${quote(entry.message)},\n`,
		)
		.join('');
	return `// The ${name} (${locale}) catalogue, written from translations/${locale}.json by \`bun run i18n:import\`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Catalogue } from '../active';

/**
 * Generated: edit \`translations/${locale}.json\` (or use Weblate) and run \`bun run i18n:import\`. A
 * message missing here shows in English. Each \`// source:\` comment records the hash of the English
 * text the translation was made from, so \`bun run i18n:status\` can tell when English has moved on.
 */
const messages: Catalogue = {
${body}};

export default messages;
`;
}

/** Lays TypeScript out with the repository's Prettier configuration. */
export async function formatTypeScript(source: string, filepath: string): Promise<string> {
	const config = await resolveConfig(filepath);
	return format(source, { ...config, filepath });
}

/** The catalogue module a translation file turns into. */
export async function buildCatalogue(
	locale: string,
	entries: CatalogueEntry[],
	filepath: string,
): Promise<string> {
	return formatTypeScript(renderCatalogue(locale, entries), filepath);
}

/* -------------------------------------------------------------------------------------- status */

export interface LocaleStatus {
	locale: string;
	/** Messages the locale is expected to have, plural forms counted. */
	total: number;
	/** Translated and made from the current English text. */
	translated: number;
	missing: string[];
	/** Translated from English text that has since changed. */
	stale: string[];
	extra: string[];
	/** The share of `total` that is translated and current, to one decimal place. */
	percent: number;
}

export function localeStatus(
	locale: string,
	english: EnglishContext,
	translated: SourceEntry[],
	tag: string,
): LocaleStatus {
	const expected = expectedKeys(english.keys, tag);
	const expectedSet = new Set(expected);
	const byKey = new Map(translated.map((entry) => [entry.key, entry]));
	const missing: string[] = [];
	const stale: string[] = [];
	let current = 0;
	for (const key of expected) {
		const entry = byKey.get(key);
		if (!entry) {
			missing.push(key);
			continue;
		}
		const recorded = recordedSource(entry.comments);
		const wanted = hashMessage(english.byKey.get(sourceKeyFor(key, english.keySet)!)!.message);
		if (recorded !== undefined && recorded !== wanted) stale.push(key);
		else current += 1;
	}
	return {
		locale,
		total: expected.length,
		translated: current,
		missing,
		stale,
		extra: translated.map((entry) => entry.key).filter((key) => !expectedSet.has(key)),
		percent: expected.length === 0 ? 100 : Math.round((current / expected.length) * 1000) / 10,
	};
}

/** What is wrong with a status for a locale that is meant to be complete. */
export function completenessProblems(status: LocaleStatus): string[] {
	const list = (keys: string[]): string => {
		const shown = keys.slice(0, 5).join(', ');
		return keys.length > 5 ? `${shown} and ${keys.length - 5} more` : shown;
	};
	const problems: string[] = [];
	if (status.missing.length > 0) {
		problems.push(
			`${status.locale} lacks ${status.missing.length} messages (${list(status.missing)})`,
		);
	}
	if (status.stale.length > 0) {
		problems.push(
			`${status.locale} has ${status.stale.length} stale messages, whose English changed (${list(status.stale)})`,
		);
	}
	if (status.extra.length > 0) {
		problems.push(`${status.locale} has ${status.extra.length} extra keys (${list(status.extra)})`);
	}
	return problems;
}

/* --------------------------------------------------------------------------- the files on disk */

/** Where a repository keeps its catalogues, and which locales those are. */
export interface Workspace {
	/** The `i18n` source directory: `messages.ts`, `catalogues/` and `translations/` live in it. */
	dir: string;
	/** Real locales other than English, each with a catalogue and a translation file. */
	locales: readonly string[];
	/** Locales not yet complete, so a gap is not a failure. */
	incomplete: readonly string[];
	/** The source language's tag, whose messages are `messages.ts`. */
	source: string;
	/** The tag `Intl.PluralRules` is given for a locale. */
	pluralTag: (locale: string) => string;
}

export const translationPath = (ws: Workspace, locale: string): string =>
	join(ws.dir, 'translations', `${locale}.json`);
export const cataloguePath = (ws: Workspace, locale: string): string =>
	join(ws.dir, 'catalogues', `${locale}.ts`);

function readIfPresent(path: string): string | undefined {
	return existsSync(path) ? readFileSync(path, 'utf8') : undefined;
}

export function readEnglish(ws: Workspace): EnglishContext {
	return englishContext(parseObjectSource(readFileSync(join(ws.dir, 'messages.ts'), 'utf8')));
}

/** The entries a locale's catalogue module holds now; none when the module does not exist yet. */
export function readCatalogue(ws: Workspace, locale: string): SourceEntry[] {
	const text = readIfPresent(cataloguePath(ws, locale));
	return text === undefined ? [] : parseObjectSource(text);
}

/** The JSON each language's translation file should hold, from the TypeScript as it is now. */
export function plannedExports(ws: Workspace): Map<string, string> {
	const english = readEnglish(ws);
	const files = new Map<string, string>();
	files.set(translationPath(ws, ws.source), stringifyTranslations(exportEnglish(english)));
	for (const locale of ws.locales) {
		files.set(
			translationPath(ws, locale),
			stringifyTranslations(
				exportTranslation(english, readCatalogue(ws, locale), ws.pluralTag(locale)),
			),
		);
	}
	return files;
}

/** The catalogue module `locale`'s translation file turns into, or the reasons it cannot. */
export async function plannedCatalogue(
	ws: Workspace,
	locale: string,
	english = readEnglish(ws),
): Promise<{ source?: string; errors: string[] }> {
	const jsonPath = translationPath(ws, locale);
	const json = readIfPresent(jsonPath);
	if (json === undefined)
		return { errors: [`${jsonPath} does not exist; run bun run i18n:export`] };
	const result = importTranslation(
		json,
		english,
		readCatalogue(ws, locale),
		ws.pluralTag(locale),
		`translations/${locale}.json`,
	);
	if (result.errors.length > 0) return { errors: result.errors };
	const source = await buildCatalogue(locale, result.entries, cataloguePath(ws, locale));
	return { source, errors: [] };
}

export function statusOf(ws: Workspace): LocaleStatus[] {
	const english = readEnglish(ws);
	return ws.locales.map((locale) =>
		localeStatus(locale, english, readCatalogue(ws, locale), ws.pluralTag(locale)),
	);
}

/** One line per locale for `i18n:status`. */
export function formatStatus(statuses: LocaleStatus[], incomplete: readonly string[]): string {
	return statuses
		.map((s) => {
			const state = incomplete.includes(s.locale) ? 'in progress' : 'must be complete';
			return `${s.locale}: ${s.translated} translated, ${s.missing.length} missing, ${s.stale.length} stale, ${s.extra.length} extra of ${s.total} (${s.percent}%) — ${state}`;
		})
		.join('\n');
}

/** Everything `check:i18n` fails on, one sentence each; empty when all is well. */
export async function checkI18n(ws: Workspace): Promise<string[]> {
	const problems: string[] = [];
	const english = readEnglish(ws);

	// The exported JSON is a pure function of the TypeScript.
	for (const [path, expected] of plannedExports(ws)) {
		const actual = readIfPresent(path);
		const name = path.slice(ws.dir.length + 1);
		if (actual === undefined) problems.push(`${name} is missing; run bun run i18n:export`);
		else if (actual !== expected) {
			problems.push(`${name} is out of date with the TypeScript; run bun run i18n:export`);
		}
	}
	const known = new Set([ws.source, ...ws.locales].map((locale) => `${locale}.json`));
	const directory = join(ws.dir, 'translations');
	for (const file of existsSync(directory) ? readdirSync(directory) : []) {
		if (file.endsWith('.json') && !known.has(file)) {
			problems.push(`translations/${file} is for a locale that is not listed in locales.ts`);
		}
	}

	for (const locale of ws.locales) {
		// Importing the JSON gives back the catalogue that is checked in.
		if (existsSync(translationPath(ws, locale))) {
			const planned = await plannedCatalogue(ws, locale, english);
			problems.push(...planned.errors);
			if (
				planned.source !== undefined &&
				planned.source !== readIfPresent(cataloguePath(ws, locale))
			) {
				problems.push(
					`catalogues/${locale}.ts differs from what bun run i18n:import writes from translations/${locale}.json`,
				);
			}
		}

		// Placeholders in the TypeScript itself, whatever the JSON says.
		const translated = readCatalogue(ws, locale);
		for (const entry of translated) {
			const sourceKey = sourceKeyFor(entry.key, english.keySet);
			if (
				sourceKey !== undefined &&
				!sameItems(placeholders(entry.message), placeholders(english.byKey.get(sourceKey)!.message))
			) {
				problems.push(`${locale} ${entry.key} has different {placeholders} from English`);
			}
		}

		if (!ws.incomplete.includes(locale)) {
			problems.push(
				...completenessProblems(localeStatus(locale, english, translated, ws.pluralTag(locale))),
			);
		}
	}
	return problems;
}
