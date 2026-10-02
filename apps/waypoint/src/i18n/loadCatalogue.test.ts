// Tests the catalogue loader: lazy per-locale loading, English fallback, plural rules and switching live
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	activeLocale,
	formatLocale,
	localeVersion,
	resetActiveLocale,
	subscribeLocale,
} from './active';
import { activateLocale, forgetCatalogues, loadCatalogue } from './loadCatalogue';
import { enMessages, t, tf, tn } from './messages';

afterEach(() => {
	forgetCatalogues();
	resetActiveLocale();
});

const developer = { systemLanguage: 'en-US', developer: true };

describe('loadCatalogue', () => {
	it('loads a locale once and shares the answer', async () => {
		const first = loadCatalogue('fr-CA');
		expect(loadCatalogue('fr-CA')).toBe(first);
		expect((await first)['settings.section.general']).toBe('Général');
	});

	it('has English in the bundle already, as an empty overlay on the source catalogue', async () => {
		expect(await loadCatalogue('en-CA')).toEqual({});
	});
});

describe('activateLocale', () => {
	it('makes t read the new catalogue', async () => {
		expect(t('settings.section.general')).toBe('General');
		await activateLocale({ setting: 'fr-CA', ...developer });
		expect(activeLocale()).toBe('fr-CA');
		expect(t('settings.section.general')).toBe('Général');
		expect(t('settings.group.startup')).toBe('Démarrage');
		expect(t('settings.group.startup')).not.toBe(enMessages['settings.group.startup']);
	});

	it('fills tokens in a translated message and in a fallback one', async () => {
		await activateLocale({ setting: 'fr-CA', ...developer });
		expect(tf('browse.capped', { shown: '1', total: '2' })).toBe(
			'Éléments affichés\u00a0: 1 sur 2',
		);
	});

	it('chooses plural forms with Intl.PluralRules for the locale, French counting zero as one', async () => {
		await activateLocale({ setting: 'fr-CA', ...developer });
		expect(tn('tabs.count', 0)).toBe('0 onglet');
		expect(tn('tabs.count', 1)).toBe('1 onglet');
		expect(tn('tabs.count', 2)).toBe('2 onglets');
		expect(tn('tabs.count', 1_000_000)).toBe(
			`${new Intl.NumberFormat('fr-CA').format(1_000_000)} d’onglets`,
		);
		await activateLocale({ setting: 'en-CA', ...developer });
		expect(tn('tabs.count', 0)).toBe('0 tabs');
		expect(tn('tabs.count', 1)).toBe('1 tab');
	});

	it('switches live: listeners hear it once the new catalogue is in, never before', async () => {
		await activateLocale({ setting: 'en-CA', ...developer });
		const seen: string[] = [];
		const stop = subscribeLocale(() => seen.push(t('settings.section.general')));
		const before = localeVersion();
		const pending = activateLocale({ setting: 'fr-CA', ...developer });
		expect(seen).toEqual([]);
		expect(t('settings.section.general')).toBe('General');
		await pending;
		expect(seen).toEqual(['Général']);
		expect(localeVersion()).toBe(before + 1);
		stop();
		await activateLocale({ setting: 'en-CA', ...developer });
		expect(seen).toEqual(['Général']);
	});

	it('follows the OS for "system", formatting with the OS locale', async () => {
		await activateLocale({ setting: 'system', systemLanguage: 'fr-FR', developer: false });
		expect(activeLocale()).toBe('fr-CA');
		expect(formatLocale()).toBe('fr-FR');
		await activateLocale({ setting: 'system', systemLanguage: 'de-DE', developer: false });
		expect(activeLocale()).toBe('en-CA');
		expect(formatLocale()).toBe('de-DE');
	});

	it('loads a pseudo-locale only for a developer build', async () => {
		await activateLocale({ setting: 'en-XA', systemLanguage: 'en-US', developer: false });
		expect(activeLocale()).toBe('en-CA');
		await activateLocale({ setting: 'en-XA', ...developer });
		expect(activeLocale()).toBe('en-XA');
		expect(t('settings.section.general')).toMatch(/^\[[^a-zA-Z]+/);
		expect(tf('browse.capped', { shown: '1', total: '2' })).toContain('1');
	});

	it('formats numbers with the chosen locale', async () => {
		await activateLocale({ setting: 'fr-CA', ...developer });
		expect(formatLocale()).toBe('fr-CA');
		expect(tn('tabs.count', 1234)).toBe(`${new Intl.NumberFormat('fr-CA').format(1234)} onglets`);
	});
});

describe('a catalogue that cannot be loaded', () => {
	it('leaves the window in English and warns', async () => {
		vi.resetModules();
		vi.doMock('./catalogues/fr-CA', () => {
			throw new Error('chunk failed');
		});
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
		const loader = await import('./loadCatalogue');
		const active = await import('./active');
		const messages = await import('./messages');
		const resolved = await loader.activateLocale({ setting: 'fr-CA', ...developer });
		expect(resolved.locale).toBe('en-CA');
		expect(active.activeLocale()).toBe('en-CA');
		expect(messages.t('settings.section.general')).toBe('General');
		expect(warn).toHaveBeenCalled();
		vi.doUnmock('./catalogues/fr-CA');
		warn.mockRestore();
	});
});
