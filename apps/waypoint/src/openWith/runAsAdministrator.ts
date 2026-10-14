// Run as Administrator for one program, script, installer or shortcut: when it is offered and how it is started
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { hasFeature, isMimeAppsError, type PluginStatus } from '@liminal-hq/plugin-mime-apps';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { showNotice } from '../app/notices';
import type { ListingSession } from '../browse/useListingSession';
import { t } from '../i18n/messages';
import type { VfsClient } from '../services/vfsClient';
import type { OpenWithClient } from './openWithClient';
import { isLocalUri, selectionUris } from './openWithUris';

/**
 * The kinds of file Windows runs through the shell's `runas` verb. This only decides what the
 * menus offer; the plugin checks again, in Rust, before anything is launched.
 */
const RUNNABLE_EXTENSIONS = ['exe', 'bat', 'cmd', 'msi', 'lnk'];

/** Whether the name ends in a runnable extension, in any case. */
export function isRunnableName(name: string): boolean {
	const dot = name.lastIndexOf('.');
	return dot > 0 && RUNNABLE_EXTENSIONS.includes(name.slice(dot + 1).toLowerCase());
}

/** Whether a listed entry is a regular file of a runnable kind (a folder or a link is not). */
export function isRunnableEntry(entry: Entry): boolean {
	return entry.kind === 'file' && isRunnableName(entry.name);
}

/**
 * Whether Run as Administrator can work here: the plugin says it can (only Windows does) and the
 * platform is Windows. It is not behind the Administrator access setting: the system's own prompt
 * does the elevating, in an installed or a portable build alike.
 */
export function runAsAdministratorOffered(
	status: PluginStatus | null,
	platform: string | undefined,
): boolean {
	return platform === 'windows' && status !== null && hasFeature(status, 'runElevated');
}

/** True when the pane's selection is exactly one regular file of a runnable kind, known from the listing's cache. */
export function runnableFileSelected(session: ListingSession | null, selected: number): boolean {
	if (!session || selected !== 1) return false;
	const { selection } = session.store.getState();
	const id = selection.kind === 'some' ? [...selection.ids][0] : undefined;
	const entry = id === undefined ? undefined : session.model.cachedEntry(id);
	return entry !== undefined && isRunnableEntry(entry);
}

/** The words for a failure; `null` for one that is not a failure (the person declined the system's prompt). */
export function runAsAdministratorFailureText(error: unknown): string | null {
	if (isMimeAppsError(error) && error.kind === 'cancelled') return null;
	// The error can name the file; the log keeps it for diagnosis, the notice never shows it.
	console.warn('could not run it as an administrator', error);
	return t('runAsAdministrator.failed');
}

/** Starts `uris` as an administrator. Resolves to whether the system accepted it; a failure goes to `say` and a declined prompt is silent. */
export async function startRunAsAdministrator(
	client: OpenWithClient,
	uris: string[],
	say: (text: string) => void = showNotice,
): Promise<boolean> {
	try {
		await client.runElevated(uris);
		return true;
	} catch (error) {
		const text = runAsAdministratorFailureText(error);
		if (text) say(text);
		return false;
	}
}

export interface RunAsAdministratorDeps {
	client: OpenWithClient;
	vfs: Pick<VfsClient, 'entryLocation'>;
	say?: (text: string) => void;
}

/** Run as Administrator for `location` when given, else the active pane's one selected file. */
export async function runRunAsAdministratorCommand(
	session: ListingSession | null,
	{ client, vfs, say = showNotice }: RunAsAdministratorDeps,
	location?: Location,
): Promise<void> {
	const uris = location
		? isLocalUri(location.uri)
			? [location.uri]
			: null
		: await selectionUris(vfs, session);
	if (!uris || uris.length !== 1) return;
	await startRunAsAdministrator(client, uris, say);
}
