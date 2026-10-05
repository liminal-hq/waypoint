// Asking for an archive's password through a job: the question, the password given to Rust, and the words for a refusal
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import { t } from '../i18n/messages';
import { isVfsError } from '../services/vfsClient';
import { isArchiveLock, type ArchiveLock } from './lockModel';

/**
 * The operations error a failure holds: a command rejects with `{ kind: 'ops', error }` (an
 * `OpsCommandError`), and a finished job's state holds the error itself.
 */
export function opsErrorOf(failure: unknown): OpsError | null {
	if (typeof failure !== 'object' || failure === null) return null;
	const wrapper = failure as { kind?: unknown; error?: unknown };
	if (wrapper.kind === 'ops' || wrapper.kind === 'queue') {
		return (wrapper.error as OpsError | undefined) ?? null;
	}
	return typeof wrapper.kind === 'string' ? (failure as OpsError) : null;
}

/** The archive password question a failed job or plan holds, or `null` when it holds another error. */
export function lockOf(failure: unknown): ArchiveLock | null {
	const error = opsErrorOf(failure);
	if (error?.kind === 'connection' && isArchiveLock(error.error)) return error.error;
	// A bare `VfsError` (the unlock command's own refusal) is a lock too.
	return isVfsError(failure) && isArchiveLock(failure) ? failure : null;
}

/** The words for a failure of giving a password or asking the provider. */
export function commandErrorMessage(failure: unknown): string {
	if (isVfsError(failure)) return vfsErrorWords(failure);
	return failure instanceof Error ? failure.message : t('archive.locked.failed');
}

function vfsErrorWords(error: VfsError): string {
	return error.kind === 'unsupported' ? error.what : t('archive.locked.failed');
}
