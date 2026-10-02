// The Open With… command: the system's chooser for one file where there is one, else the application chooser dialog, else the default
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import type { VfsClient } from '../services/vfsClient';
import type { ListingSession } from '../browse/useListingSession';
import { showNotice } from '../app/notices';
import { openWithAbilities, type OpenWithAbilities } from './OpenWithContext';
import type { OpenWithClient } from './openWithClient';
import type { OpenWithChooserStore } from './openWithChooserStore';
import { selectionUris } from './openWithUris';
import { startOpenWith } from './startOpenWith';
import type { PluginStatus } from '@liminal-hq/plugin-mime-apps';

export interface OpenWithCommandDeps {
	client: OpenWithClient;
	status: PluginStatus | null;
	vfs: Pick<VfsClient, 'entryLocation'>;
	chooser: OpenWithChooserStore;
	say?: (text: string) => void;
}

/** Whether the command has a way to work here: the plugin can list applications or has a chooser of its own. */
export function openWithCommandAvailable(abilities: OpenWithAbilities): boolean {
	return abilities.list || abilities.chooser;
}

/**
 * Open With… for the active pane's selection. One file on a system with its own chooser (Windows,
 * a Flatpak) goes to that chooser; otherwise the dialog lists every application for the type. It
 * does nothing for a selection that is not on this computer or of more than one type.
 */
export async function runOpenWithCommand(
	session: ListingSession | null,
	{ client, status, vfs, chooser, say = showNotice }: OpenWithCommandDeps,
): Promise<void> {
	const abilities = openWithAbilities(status);
	if (!openWithCommandAvailable(abilities)) return;
	const uris = await selectionUris(vfs, session);
	if (!uris) return;
	if (abilities.chooser && uris.length === 1) {
		await startOpenWith(client, uris, { kind: 'chooser' }, say);
		return;
	}
	try {
		const handlers = await client.handlers(uris);
		if (handlers.mixed) {
			say(t('openWith.mixed'));
			return;
		}
		chooser.getState().open({ uris, handlers, scope: 'all' });
	} catch (error) {
		console.warn('could not read the applications', error);
		say(t('openWith.failed.list'));
	}
}
