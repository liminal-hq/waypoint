// Telling a password question about an archive from a server's own login question
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { isArchiveLocation } from './archiveNames';

/** An archive that needs its password, or refused the one it was given. */
export type ArchiveLock = Extract<VfsError, { kind: 'authRequired' | 'authFailed' }>;

/**
 * Whether `error` is the archive provider asking for a password: the provider puts the `archive:`
 * location of the archive in the error, where a server's own login question has the server's (an
 * archive on a server is told from the server by it).
 */
export function isArchiveLock(error: VfsError): error is ArchiveLock {
	return (
		(error.kind === 'authRequired' || error.kind === 'authFailed') &&
		isArchiveLocation(error.location)
	);
}

/** The archive's name for a title: the last part of what the error says it is. */
export function lockedName(location: Location): string {
	const parts = location.display.split(/[\\/›]/).map((part) => part.trim());
	return parts.filter((part) => part !== '').at(-1) ?? location.display;
}
