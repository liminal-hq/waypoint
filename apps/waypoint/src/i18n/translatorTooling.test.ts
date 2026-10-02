// Tests the translator tooling: export format and notes, the import round trip, stale and status, and the gate
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import {
	cataloguePath,
	checkI18n,
	englishContext,
	exportEnglish,
	exportTranslation,
	hashMessage,
	importTranslation,
	localeStatus,
	noteFor,
	parseObjectSource,
	plannedCatalogue,
	plannedExports,
	readCatalogue,
	recordedSource,
	renderCatalogue,
	statusOf,
	stringifyTranslations,
	translationPath,
	translatorComment,
	type Workspace,
} from '../../../../scripts/i18nTooling';
import { repositoryWorkspace } from '../../../../scripts/i18nWorkspace';
import { enMessages } from './messages';

const ENGLISH = `// header
export const enMessages = {
	// translator: The file’s name, as a column heading.
	// Keep it short.
	'a.name': 'Name',
	'a.greeting': 'Hello {name}, you have {count}',
	// translator: A short heading for the group of tabs.
	'a.tabs.one': '{count} tab',
	'a.tabs.other': '{count} tabs',
	'a.misc.other': 'Other',
	'a.quote': "It's “quoted”",
} as const;
`;

const english = englishContext(parseObjectSource(ENGLISH));
const dirs: string[] = [];
const repoRoot = join(import.meta.dirname, '../../../..');

/** A scratch `i18n` directory holding `ENGLISH` and a workspace for `fr-CA` and `pl-PL`. */
function scratch(incomplete: string[] = []): Workspace {
	const dir = mkdtempSync(join(tmpdir(), 'waypoint-i18n-'));
	dirs.push(dir);
	mkdirSync(join(dir, 'catalogues'));
	mkdirSync(join(dir, 'translations'));
	writeFileSync(join(dir, 'messages.ts'), ENGLISH);
	// Prettier looks for its configuration beside the file, so give the scratch directory the repository's.
	writeFileSync(join(dir, '.prettierrc'), readFileSync(join(repoRoot, '.prettierrc')));
	return {
		dir,
		locales: ['fr-CA', 'pl-PL'],
		incomplete,
		source: 'en-CA',
		pluralTag: (locale) => (locale === 'en-CA' ? 'en' : locale),
	};
}

afterEach(() => {
	for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

describe('reading a catalogue source', () => {
	it('reads keys, messages and the comments above each key', () => {
		expect(english.keys).toEqual([
			'a.name',
			'a.greeting',
			'a.tabs.one',
			'a.tabs.other',
			'a.misc.other',
			'a.quote',
		]);
		expect(english.byKey.get('a.quote')?.message).toBe('It’s “quoted”'.replace('It’s', "It's"));
		expect(english.byKey.get('a.name')?.comments).toEqual([
			'translator: The file’s name, as a column heading.',
			'Keep it short.',
		]);
	});

	it('unescapes strings and refuses a message that is not one string', () => {
		const parsed = parseObjectSource(
			String.raw`const m = { 'k': 'a\'b\\c\ndé\u{1f600}', "j": "x" };`,
		);
		expect(parsed.map((entry) => entry.message)).toEqual(["a'b\\c\ndé\u{1f600}", 'x']);
		expect(() => parseObjectSource("const m = { 'k': 'a' + 'b' };")).toThrow(/joins strings/);
		expect(() => parseObjectSource('const m = { k: "a" };')).toThrow(/quoted string/);
		expect(() => parseObjectSource('nothing here')).toThrow(/object literal/);
	});

	it('finds the translator comment, continued over plain comment lines, and a recorded hash', () => {
		expect(translatorComment(['translator: One', 'two', 'source: abcdef01'])).toBe('One two');
		expect(translatorComment(['just a comment'])).toBeUndefined();
		expect(recordedSource(['source: abcdef01'])).toBe('abcdef01');
		expect(recordedSource(['source: nope'])).toBeUndefined();
	});
});

describe('notes', () => {
	it('give the area, the placeholders with their meanings, and the translator comment', () => {
		expect(noteFor('a.name', english, 'en')).toBe(
			'Area: a. The file’s name, as a column heading. Keep it short.',
		);
		const note = noteFor('a.greeting', english, 'en');
		expect(note).toContain('Area: a.');
		expect(note).toContain('{name} is the name of a file, folder, tab or other item');
		expect(note).toContain('{count} is a number of items');
	});

	it('say a message is one of a plural group and name its siblings in the language’s forms', () => {
		expect(noteFor('a.tabs.one', english, 'en')).toContain(
			'Plural: one of a group; the other forms are a.tabs.other.',
		);
		expect(noteFor('a.tabs.one', english, 'pl')).toContain(
			'the other forms are a.tabs.few, a.tabs.many, a.tabs.other.',
		);
		// A form English has no key for takes the group's comment.
		expect(noteFor('a.tabs.few', english, 'pl')).toContain(
			'A short heading for the group of tabs.',
		);
	});

	it('leave a lone other as a plain message', () => {
		expect(noteFor('a.misc.other', english, 'en')).not.toContain('Plural');
	});

	it('mark a key English does not have', () => {
		expect(noteFor('a.nope', english, 'en')).toContain('Not a message English has');
	});
});

describe('export', () => {
	it('writes English in source order with a message and a note, and no hash', () => {
		const out = exportEnglish(english);
		expect(Object.keys(out)).toEqual(english.keys);
		expect(Object.keys(out['a.name']!)).toEqual(['message', 'description']);
		const text = stringifyTranslations(out);
		expect(text.startsWith('{\n  "a.name": {\n    "message": "Name",')).toBe(true);
		expect(text.endsWith('}\n')).toBe(true);
	});

	it('writes a translation in English order, plural forms in place, with the hash it records', () => {
		const translated = parseObjectSource(`const m = {
	// source: 00000000
	'a.tabs.many': 'x',
	'a.name': 'Nom',
	'a.tabs.one': 'un',
	'zzz.extra': 'e',
};`);
		const out = exportTranslation(english, translated, 'pl');
		expect(Object.keys(out)).toEqual(['a.name', 'a.tabs.one', 'a.tabs.many', 'zzz.extra']);
		expect(out['a.tabs.many']!.source).toBe('00000000');
		expect(out['a.name']!.source).toBe(hashMessage('Name'));
		expect(out['zzz.extra']!.description).toContain('Not a message English has');
	});
});

describe('import', () => {
	const make = (entries: Record<string, unknown>): string => JSON.stringify(entries);

	it('writes the catalogue in English order with a source comment per key', () => {
		const result = importTranslation(
			make({ 'a.greeting': { message: 'Bonjour {name}, {count}' }, 'a.name': { message: 'Nom' } }),
			english,
			[],
			'fr-CA',
		);
		expect(result.errors).toEqual([]);
		expect(result.entries.map((entry) => entry.key)).toEqual(['a.name', 'a.greeting']);
		expect(result.entries[0]!.source).toBe(hashMessage('Name'));
		expect(renderCatalogue('fr-CA', result.entries)).toContain('// source: ');
	});

	it('refuses a placeholder mismatch and an unknown key, naming each', () => {
		const result = importTranslation(
			make({
				'a.greeting': { message: 'Bonjour {nom}, {count}' },
				'a.nope': { message: 'x' },
				'a.tabs.few': { message: 'x' },
			}),
			english,
			[],
			'en',
			'en.json',
		);
		expect(result.errors).toEqual([
			'en.json: a.greeting has placeholders {count}, {nom} but English has {count}, {name}',
			'en.json: a.nope is not a message English has for en',
			'en.json: a.tabs.few is not a message English has for en',
		]);
	});

	it('accepts the extra plural forms of a language and leaves an empty message untranslated', () => {
		const result = importTranslation(
			make({
				'a.tabs.few': { message: '{count} karty' },
				'a.tabs.many': { message: '{count} kart' },
				'a.name': { message: '' },
			}),
			english,
			[],
			'pl',
		);
		expect(result.errors).toEqual([]);
		expect(result.entries.map((entry) => entry.key)).toEqual(['a.tabs.few', 'a.tabs.many']);
		expect(result.entries[0]!.source).toBe(hashMessage('{count} tabs'));
	});

	it('rejects JSON that is not an object of entries', () => {
		expect(importTranslation('[1]', english, [], 'en').errors).toHaveLength(1);
		expect(importTranslation('{', english, [], 'en').errors[0]).toContain('not valid JSON');
		expect(importTranslation(make({ 'a.name': {} }), english, [], 'en').errors[0]).toContain(
			'no "message"',
		);
	});

	it('keeps the recorded hash when the file drops it and the message is unchanged', () => {
		const existing = parseObjectSource("const m = {\n// source: 12345678\n'a.name': 'Nom',\n};");
		const kept = importTranslation(
			make({ 'a.name': { message: 'Nom' } }),
			english,
			existing,
			'fr-CA',
		);
		expect(kept.entries[0]!.source).toBe('12345678');
		const edited = importTranslation(
			make({ 'a.name': { message: 'Prénom' } }),
			english,
			existing,
			'fr-CA',
		);
		expect(edited.entries[0]!.source).toBe(hashMessage('Name'));
		const given = importTranslation(
			make({ 'a.name': { message: 'Nom', source: 'abcdef01' } }),
			english,
			existing,
			'fr-CA',
		);
		expect(given.entries[0]!.source).toBe('abcdef01');
	});
});

describe('the round trip', () => {
	const TRICKY = {
		'a.name': { message: 'Nom' },
		'a.greeting': { message: 'L\'équipe: « {name} », {count}\nLigne 2 \\ "fin" 😀 ’' },
		'a.quote': { message: "C'est “cité”" },
		'a.tabs.one': { message: '{count} onglet' },
		'a.tabs.many': { message: '{count} d’onglets' },
		'a.tabs.other': { message: '{count} onglets' },
	};

	it('changes nothing: export then import then export is byte for byte the same', async () => {
		const ws = scratch(['fr-CA', 'pl-PL']);
		writeFileSync(translationPath(ws, 'fr-CA'), JSON.stringify(TRICKY));
		writeFileSync(translationPath(ws, 'pl-PL'), '{}');
		for (const locale of ws.locales) {
			const first = await plannedCatalogue(ws, locale);
			expect(first.errors).toEqual([]);
			writeFileSync(cataloguePath(ws, locale), first.source!);
		}
		const exported = plannedExports(ws);
		for (const [path, content] of exported) writeFileSync(path, content);

		const second = await plannedCatalogue(ws, 'fr-CA');
		expect(second.source).toBe(readFileSync(cataloguePath(ws, 'fr-CA'), 'utf8'));
		const again = plannedExports(ws);
		for (const [path, content] of exported) expect(again.get(path)).toBe(content);

		const roundTripped = readCatalogue(ws, 'fr-CA');
		expect(roundTripped.find((entry) => entry.key === 'a.greeting')!.message).toBe(
			TRICKY['a.greeting'].message,
		);
		expect(await checkI18n(ws)).toEqual([]);
	});

	it('writes a catalogue with the licence header, a typed default export and Prettier’s layout', async () => {
		const ws = scratch();
		writeFileSync(translationPath(ws, 'fr-CA'), JSON.stringify(TRICKY));
		const { source } = await plannedCatalogue(ws, 'fr-CA');
		expect(source).toContain('// (c) Copyright 2026 Liminal HQ, Scott Morris');
		expect(source).toContain('// SPDX-License-Identifier: Apache-2.0 OR MIT');
		expect(source).toContain("import type { Catalogue } from '../active';");
		expect(source).toContain('export default messages;\n');
		expect(source).toContain('\t// source: ');
		expect(source!.endsWith('\n')).toBe(true);
	});
});

describe('stale messages and the status', () => {
	it('flags a translation whose English text has changed', () => {
		const translated = parseObjectSource(`const m = {
	// source: ${hashMessage('Name')}
	'a.name': 'Nom',
	// source: ${hashMessage('Hello {name}, an older wording')}
	'a.greeting': 'Bonjour {name}, {count}',
};`);
		const status = localeStatus('fr-CA', english, translated, 'fr-CA');
		expect(status.stale).toEqual(['a.greeting']);
		expect(status.translated).toBe(1);
	});

	it('counts translated, missing, stale and extra, and the percentage that is current', () => {
		const translated = parseObjectSource(`const m = {
	// source: ${hashMessage('Name')}
	'a.name': 'Nom',
	// source: 00000000
	'a.quote': 'x',
	'a.tabs.one': 'un',
	'a.tabs.few': 'trop',
};`);
		const status = localeStatus('fr-CA', english, translated, 'fr-CA');
		// fr-CA wants one, many and other of the group: six English keys become seven.
		expect(status.total).toBe(7);
		expect(status.translated).toBe(2);
		expect(status.stale).toEqual(['a.quote']);
		expect(status.extra).toEqual(['a.tabs.few']);
		expect(status.missing).toEqual(['a.greeting', 'a.tabs.many', 'a.tabs.other', 'a.misc.other']);
		expect(status.percent).toBe(28.6);
	});

	it('reports a translation made from changed English after the English moves on', async () => {
		const ws = scratch(['fr-CA', 'pl-PL']);
		writeFileSync(translationPath(ws, 'fr-CA'), JSON.stringify({ 'a.name': { message: 'Nom' } }));
		writeFileSync(translationPath(ws, 'pl-PL'), '{}');
		for (const locale of ws.locales) {
			writeFileSync(cataloguePath(ws, locale), (await plannedCatalogue(ws, locale)).source!);
		}
		expect(statusOf(ws).find((s) => s.locale === 'fr-CA')!.stale).toEqual([]);
		writeFileSync(join(ws.dir, 'messages.ts'), ENGLISH.replace("'Name'", "'Full name'"));
		expect(statusOf(ws).find((s) => s.locale === 'fr-CA')!.stale).toEqual(['a.name']);
	});
});

describe('check:i18n', () => {
	async function fresh(incomplete: string[]): Promise<Workspace> {
		const ws = scratch(incomplete);
		writeFileSync(translationPath(ws, 'fr-CA'), JSON.stringify({ 'a.name': { message: 'Nom' } }));
		writeFileSync(translationPath(ws, 'pl-PL'), '{}');
		for (const locale of ws.locales) {
			writeFileSync(cataloguePath(ws, locale), (await plannedCatalogue(ws, locale)).source!);
		}
		for (const [path, content] of plannedExports(ws)) writeFileSync(path, content);
		return ws;
	}

	it('passes when everything agrees and the incomplete locales have gaps', async () => {
		expect(await checkI18n(await fresh(['fr-CA', 'pl-PL']))).toEqual([]);
	});

	it('fails for a locale that is not listed as incomplete and has gaps', async () => {
		const problems = await checkI18n(await fresh(['pl-PL']));
		expect(problems).toHaveLength(1);
		expect(problems[0]).toMatch(/^fr-CA lacks 6 messages/);
	});

	it('fails when the exported JSON is out of date with the TypeScript', async () => {
		const ws = await fresh(['fr-CA', 'pl-PL']);
		writeFileSync(join(ws.dir, 'messages.ts'), ENGLISH.replace("'Name'", "'Full name'"));
		const problems = await checkI18n(ws);
		expect(problems).toContain(
			'translations/en-CA.json is out of date with the TypeScript; run bun run i18n:export',
		);
	});

	it('fails when importing would change a catalogue, and when a placeholder differs', async () => {
		const ws = await fresh(['fr-CA', 'pl-PL']);
		writeFileSync(
			cataloguePath(ws, 'fr-CA'),
			readFileSync(cataloguePath(ws, 'fr-CA'), 'utf8').replace("'Nom'", "'Nom!'"),
		);
		const changed = await checkI18n(ws);
		expect(
			changed.some((p) => p.includes('catalogues/fr-CA.ts differs from what bun run i18n:import')),
		).toBe(true);

		const bad = await fresh(['fr-CA', 'pl-PL']);
		writeFileSync(
			cataloguePath(bad, 'fr-CA'),
			readFileSync(cataloguePath(bad, 'fr-CA'), 'utf8').replace(
				"'a.name': 'Nom'",
				"'a.greeting': '{nom}',\n\t'a.name': 'Nom'",
			),
		);
		expect(
			(await checkI18n(bad)).some((p) =>
				p.includes('fr-CA a.greeting has different {placeholders}'),
			),
		).toBe(true);
	});

	it('fails on a stale message in a locale that is meant to be complete', async () => {
		const ws = await fresh(['pl-PL']);
		const all = Object.fromEntries(
			[
				'a.name',
				'a.greeting',
				'a.tabs.one',
				'a.tabs.many',
				'a.tabs.other',
				'a.misc.other',
				'a.quote',
			].map((key) => [
				key,
				{
					message:
						key === 'a.greeting' ? '{name} {count}' : key.includes('tabs') ? '{count} x' : 'x',
				},
			]),
		);
		writeFileSync(translationPath(ws, 'fr-CA'), JSON.stringify(all));
		writeFileSync(cataloguePath(ws, 'fr-CA'), (await plannedCatalogue(ws, 'fr-CA')).source!);
		for (const [path, content] of plannedExports(ws)) writeFileSync(path, content);
		expect(await checkI18n(ws)).toEqual([]);
		writeFileSync(join(ws.dir, 'messages.ts'), ENGLISH.replace("'Name'", "'Full name'"));
		for (const [path, content] of plannedExports(ws)) writeFileSync(path, content);
		const problems = await checkI18n(ws);
		expect(problems).toEqual(['fr-CA has 1 stale messages, whose English changed (a.name)']);
	});
});

describe('the checked-in translation files', () => {
	const ws = repositoryWorkspace();

	it('match a fresh export of the TypeScript', () => {
		for (const [path, content] of plannedExports(ws)) {
			expect(readFileSync(path, 'utf8'), path).toBe(content);
		}
	});

	it('hold every English message, in source order, in en-CA.json', () => {
		const file = JSON.parse(readFileSync(translationPath(ws, 'en-CA'), 'utf8')) as Record<
			string,
			{ message: string; description: string }
		>;
		expect(Object.keys(file)).toEqual(Object.keys(enMessages));
		expect(file['browse.column.kind']!.description).toContain('Not the file extension');
	});

	it('pass the whole check', async () => {
		expect(await checkI18n(ws)).toEqual([]);
	});

	it('carry a translator comment on the short ambiguous words', () => {
		const source = readFileSync(join(ws.dir, 'messages.ts'), 'utf8');
		const comments = parseObjectSource(source).filter((entry) => translatorComment(entry.comments));
		expect(comments.length).toBeGreaterThanOrEqual(20);
		const keys = comments.map((entry) => entry.key);
		for (const key of [
			'browse.column.name',
			'browse.column.size',
			'menu.open',
			'menu.groupBy',
			'app.name',
		]) {
			expect(keys).toContain(key);
		}
	});
});
