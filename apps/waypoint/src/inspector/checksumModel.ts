// The pure rules of the Properties window's checksum: who may be hashed, what is warned about, and how a run reads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EntryKind } from '@liminal-hq/waypoint-protocol/generated/EntryKind';
import type { VerifyAlgorithm } from '@liminal-hq/waypoint-protocol/generated/VerifyAlgorithm';
import type { MessageId } from '../i18n/messages';

/** A file larger than this is warned about before its checksum is read: it is read in full, so it takes a while. */
export const LARGE_FILE_BYTES = 1024 * 1024 * 1024;

/** The algorithm a checksum starts as. */
export const DEFAULT_ALGORITHM: VerifyAlgorithm = 'sha256';

/** Whether a checksum is offered for an entry: a regular file, or a link that leads to one (never a folder, a pipe or a device). */
export function canChecksum(kind: EntryKind, resolvesTo: EntryKind | null): boolean {
	return kind === 'file' || (kind === 'symlink' && resolvesTo === 'file');
}

/** Whether to warn before calculating: the size is known and above 1 GiB. */
export function needsSizeWarning(size: number | null): boolean {
	return size !== null && size > LARGE_FILE_BYTES;
}

const ALGORITHM_LABEL: Record<VerifyAlgorithm, MessageId> = {
	sha256: 'checksum.algorithm.sha256',
	blake3: 'checksum.algorithm.blake3',
};

/** The message that names an algorithm. */
export function algorithmMessage(algorithm: VerifyAlgorithm): MessageId {
	return ALGORITHM_LABEL[algorithm];
}

/** How far a run is, from 0 to 1; `0` for a file of no size, so an empty file does not divide by zero. */
export function fraction(bytesRead: number, total: number): number {
	if (total <= 0) return 0;
	return Math.min(1, Math.max(0, bytesRead / total));
}
