// Tests the locale root: nothing draws before the catalogue, no untranslated flash, and a live switch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { resetActiveLocale, useLocaleVersion } from './active';
import { forgetCatalogues } from './loadCatalogue';
import { LocaleRoot } from './LocaleRoot';
import { t } from './messages';
import { useState } from 'react';

afterEach(() => {
	cleanup();
	forgetCatalogues();
	resetActiveLocale();
});

const withLanguage = (language: string) => ({
	...DEFAULT_SETTINGS,
	locale: { ...DEFAULT_SETTINGS.locale, language },
});

/** Records the text of every render, so a flash of English before French would show. */
const renders: string[] = [];
function Probe() {
	const [mounts] = useState(() => ({ n: 0 }));
	mounts.n += 1;
	renders.push(t('settings.section.general'));
	return <p data-testid="probe">{t('settings.section.general')}</p>;
}

describe('LocaleRoot', () => {
	it('draws nothing until the settings and the catalogue are in, then draws in the language at once', async () => {
		renders.length = 0;
		const fake = createFakeSettingsClient(withLanguage('fr-CA'));
		const hold = fake.holdSnapshot();
		render(
			<LocaleRoot client={fake} systemLanguage="en-US" developer>
				<Probe />
			</LocaleRoot>,
		);
		expect(screen.queryByTestId('probe')).toBeNull();
		await act(async () => hold.release());
		expect(await screen.findByTestId('probe')).toHaveTextContent('Général');
		expect(renders.every((text) => text === 'Général')).toBe(true);
	});

	it('uses the OS language for "system", and English when it has no catalogue', async () => {
		const fake = createFakeSettingsClient();
		render(
			<LocaleRoot client={fake} systemLanguage="fr-FR" developer>
				<Probe />
			</LocaleRoot>,
		);
		expect(await screen.findByTestId('probe')).toHaveTextContent('Général');
		cleanup();
		resetActiveLocale();
		render(
			<LocaleRoot client={createFakeSettingsClient()} systemLanguage="de-DE" developer>
				<Probe />
			</LocaleRoot>,
		);
		expect(await screen.findByTestId('probe')).toHaveTextContent('General');
	});

	it('switches live when the setting changes, with no frame in between', async () => {
		renders.length = 0;
		const fake = createFakeSettingsClient(withLanguage('en-CA'));
		render(
			<LocaleRoot client={fake} systemLanguage="en-US" developer>
				<Probe />
			</LocaleRoot>,
		);
		expect(await screen.findByTestId('probe')).toHaveTextContent('General');
		await act(async () => {
			fake.change(withLanguage('fr-CA'));
		});
		expect(await screen.findByText('Général')).toBeInTheDocument();
		expect(renders.at(-1)).toBe('Général');
		await act(async () => {
			fake.change(withLanguage('en-XA'));
		});
		await waitFor(() => expect(screen.getByTestId('probe')).toHaveTextContent(/^\[/));
	});

	it('keeps its state when asked not to rebuild, for a screen that re-renders itself', async () => {
		const fake = createFakeSettingsClient(withLanguage('en-CA'));
		let mounted = 0;
		function Counter() {
			useLocaleVersion();
			useState(() => {
				mounted += 1;
			});
			return <p data-testid="probe">{t('settings.section.general')}</p>;
		}
		render(
			<LocaleRoot client={fake} systemLanguage="en-US" developer rebuild={false}>
				<Counter />
			</LocaleRoot>,
		);
		await screen.findByTestId('probe');
		await act(async () => {
			fake.change(withLanguage('fr-CA'));
		});
		await waitFor(() => expect(screen.getByTestId('probe')).toHaveTextContent('Général'));
		expect(mounted).toBe(1);
	});

	it('starts in English when the settings cannot be read', async () => {
		const fake = createFakeSettingsClient();
		fake.snapshot = () => Promise.reject(new Error('no plugin'));
		render(
			<LocaleRoot client={fake} systemLanguage="en-US" developer>
				<Probe />
			</LocaleRoot>,
		);
		expect(await screen.findByTestId('probe')).toHaveTextContent('General');
	});
});
