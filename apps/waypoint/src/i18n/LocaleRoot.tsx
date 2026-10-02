// Brings in the catalogue for the language setting before a window draws, and follows changes live
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Fragment, useEffect, useState, type ReactNode } from 'react';
import { DEFAULT_SETTINGS, type SettingsClient } from '../services/settingsClient';
import { createTauriSettingsClient } from '../services/tauriSettingsClient';
import { useLocaleVersion } from './active';
import { activateLocale } from './loadCatalogue';

interface LocaleRootProps {
	/** The settings plugin's client; the real one unless a test supplies its own. */
	client?: SettingsClient;
	/** The OS language; the webview's `navigator.language` unless supplied. */
	systemLanguage?: string;
	/** Whether the pseudo-locales exist: a developer build unless supplied. */
	developer?: boolean;
	/**
	 * Whether a language change rebuilds the content (the default). A screen that re-renders itself
	 * on `useLocaleVersion` passes `false`, so it keeps its own state, such as the open page.
	 */
	rebuild?: boolean;
	children: ReactNode;
}

function webSystemLanguage(): string {
	const tag = globalThis.navigator?.language;
	return tag && /^[a-zA-Z]{2,3}(-[a-zA-Z0-9]+)*$/.test(tag) ? tag : 'en-CA';
}

/**
 * Mounted once per window, inside the theme root. It draws nothing until the first answer from the
 * settings (or its failure) and the catalogue for the language it names are in, so no window shows
 * untranslated text and then switches (A68). When the language setting changes it loads the new
 * catalogue first and then rebuilds the window's content in it: components read their messages
 * with `t()` while they render, so a new language needs a new render of all of them.
 */
export function LocaleRoot({
	client,
	systemLanguage,
	developer,
	rebuild = true,
	children,
}: LocaleRootProps) {
	const [settingsClient] = useState(() => client ?? createTauriSettingsClient());
	const [language, setLanguage] = useState<string | null>(null);
	const [loadedFor, setLoadedFor] = useState<string | null>(null);
	const version = useLocaleVersion();
	const isDeveloper = developer ?? import.meta.env.DEV;
	const system = systemLanguage ?? webSystemLanguage();

	useEffect(() => {
		let live = true;
		let revision = -1;
		const take = (snapshot: { revision: number; settings: typeof DEFAULT_SETTINGS }): void => {
			if (live && snapshot.revision > revision) {
				revision = snapshot.revision;
				setLanguage(snapshot.settings.locale.language);
			}
		};
		settingsClient.snapshot().then(take, () => live && setLanguage((now) => now ?? 'system'));
		const stop = settingsClient.onChanged(take);
		return () => {
			live = false;
			stop();
		};
	}, [settingsClient]);

	useEffect(() => {
		if (language === null) return;
		let live = true;
		void activateLocale({ setting: language, systemLanguage: system, developer: isDeveloper }).then(
			() => live && setLoadedFor(language),
		);
		return () => {
			live = false;
		};
	}, [language, system, isDeveloper]);

	// Once one catalogue is in, a change keeps the old content on screen until the new one is ready.
	if (loadedFor === null) return null;
	return <Fragment key={rebuild ? version : 0}>{children}</Fragment>;
}
