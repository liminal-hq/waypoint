// What the destination dialog decides apart from drawing: what a typed folder came to, and the recent folders it offers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { schemeLabel } from '../connections/connectModel';
import { isVfsError, type VfsClient } from '../services/vfsClient';
import type { Location } from '../services/opsClient';
import { t, tf } from '../i18n/messages';
import { normaliseUri } from './clipboardRules';

/** What checking a typed or chosen folder found. */
export type DestinationCheck =
	{ state: 'ok'; location: Location } | { state: 'problem'; message: string };

function baseName(display: string): string {
	const parts = display.split('/').filter(Boolean);
	return parts[parts.length - 1] ?? display;
}

/**
 * Turns `text` into a folder that can be written to, or says in words why not. Rust parses the text
 * (`~`, relative paths against `base`, `file://` URIs) and Rust checks the folder: nothing here
 * splits or joins a path. A problem is a sentence for the field's status line.
 */
export async function checkDestination(
	vfs: Pick<VfsClient, 'parseLocation' | 'checkFolder'>,
	text: string,
	base: Location,
	options: { origin?: Location | undefined; forbidOrigin?: boolean | undefined } = {},
): Promise<DestinationCheck> {
	const input = text.trim();
	if (input === '') return { state: 'problem', message: t('destination.check.empty') };
	let location: Location;
	try {
		location = await vfs.parseLocation(input, base);
	} catch (error) {
		if (isVfsError(error) && error.kind === 'unsupported') {
			return {
				state: 'problem',
				message: tf('destination.check.unsupported', { what: error.what }),
			};
		}
		if (isVfsError(error) && error.kind === 'protocolOff') {
			return {
				state: 'problem',
				message: tf('destination.check.protocolOff', { protocol: schemeLabel(error.scheme) }),
			};
		}
		return { state: 'problem', message: tf('destination.check.invalid', { input }) };
	}
	const name = baseName(location.display) || location.display;
	try {
		const check = await vfs.checkFolder(location);
		if (!check.isFolder) {
			return { state: 'problem', message: tf('destination.check.notFolder', { name }) };
		}
		if (!check.writable) {
			return { state: 'problem', message: tf('destination.check.readOnly', { name }) };
		}
	} catch (error) {
		const kind = isVfsError(error) ? error.kind : null;
		const reason =
			kind === 'notFound'
				? tf('destination.check.notFound', { name })
				: kind === 'permissionDenied'
					? tf('destination.check.denied', { name })
					: kind === 'notADirectory'
						? tf('destination.check.notFolder', { name })
						: t('destination.check.failed');
		return { state: 'problem', message: reason };
	}
	if (
		options.forbidOrigin &&
		options.origin &&
		normaliseUri(options.origin.uri) === normaliseUri(location.uri)
	) {
		return { state: 'problem', message: t('destination.check.sameFolder') };
	}
	return { state: 'ok', location };
}

// --- Recent destinations ---------------------------------------------------------------------

/** How many recent destinations are kept. */
export const RECENT_LIMIT = 8;

const STORAGE_KEY = 'waypoint.recentDestinations';

/** A small list of the folders recently chosen in the dialog, newest first, kept in this window's page storage. */
export interface RecentDestinations {
	list(): Location[];
	remember(location: Location): void;
}

function readStored(storage: Pick<Storage, 'getItem'> | null): Location[] {
	try {
		const parsed: unknown = JSON.parse(storage?.getItem(STORAGE_KEY) ?? '[]');
		if (!Array.isArray(parsed)) return [];
		return parsed
			.filter(
				(item): item is Location =>
					typeof item === 'object' &&
					item !== null &&
					typeof (item as Location).uri === 'string' &&
					typeof (item as Location).display === 'string',
			)
			.slice(0, RECENT_LIMIT);
	} catch {
		return [];
	}
}

function defaultStorage(): Storage | null {
	try {
		return typeof localStorage === 'undefined' ? null : localStorage;
	} catch {
		return null;
	}
}

/**
 * Recent destinations, newest first and without repeats. They are a convenience of this page,
 * not state anyone relies on: where storage is blocked or empty the list is just empty.
 */
export function createRecentDestinations(
	storage: Pick<Storage, 'getItem' | 'setItem'> | null = defaultStorage(),
): RecentDestinations {
	let held = readStored(storage);
	return {
		list: () => [...held],
		remember(location) {
			held = [
				location,
				...held.filter((item) => normaliseUri(item.uri) !== normaliseUri(location.uri)),
			].slice(0, RECENT_LIMIT);
			try {
				storage?.setItem(STORAGE_KEY, JSON.stringify(held));
			} catch {
				// Nothing relies on this list, so a full or blocked store is not worth a message.
			}
		},
	};
}

/** The window's own recent destinations. */
export const recentDestinations = createRecentDestinations();
