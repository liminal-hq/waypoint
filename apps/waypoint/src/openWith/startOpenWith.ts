// Starts an application on some locations and says, in words, when it could not
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { isMimeAppsError, type App } from '@liminal-hq/plugin-mime-apps';
import { t, tf } from '../i18n/messages';
import type { OpenWithClient } from './openWithClient';

/** How the locations are to be opened. */
export type OpenWithChoice = { kind: 'default' } | { kind: 'app'; app: App } | { kind: 'chooser' };

/** The words for a failure; `null` for one that is not a failure (the person dismissed the system's chooser). */
export function openWithFailureText(error: unknown, app: App | null): string | null {
	if (isMimeAppsError(error)) {
		if (error.kind === 'cancelled') return null;
		if (error.kind === 'noHandler') return t('openWith.noHandler');
	}
	console.warn('could not open with an application', error);
	return app ? tf('openWith.failed.app', { app: app.name }) : t('openWith.failed');
}

/**
 * Opens `uris` as chosen. Resolves to whether something was started; a failure goes to `say`
 * (a dismissed system chooser is silent) and resolves to `false`.
 */
export async function startOpenWith(
	client: OpenWithClient,
	uris: string[],
	choice: OpenWithChoice,
	say: (text: string) => void,
): Promise<boolean> {
	try {
		if (choice.kind === 'default') await client.openDefault(uris);
		else if (choice.kind === 'chooser') await client.choose(uris);
		else await client.openWith(uris, choice.app.id);
		return true;
	} catch (error) {
		const text = openWithFailureText(error, choice.kind === 'app' ? choice.app : null);
		if (text) say(text);
		return false;
	}
}
