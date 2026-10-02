// The frontend's view of a file checksum: started on request, streamed, cancellable
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ChecksumEvent } from '@liminal-hq/waypoint-protocol/generated/ChecksumEvent';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { VerifyAlgorithm } from '@liminal-hq/waypoint-protocol/generated/VerifyAlgorithm';

/** The hashes the verification offers, in the order the choice lists them (SHA-256 first, the default). */
export const CHECKSUM_ALGORITHMS: readonly VerifyAlgorithm[] = ['sha256', 'blake3'];

/** A running checksum. */
export interface ChecksumRun {
	readonly job: number;
	/** Stops the run; it ends with a `cancelled` event. */
	cancel(): Promise<void>;
}

/**
 * Hashes one local file of a listing this window opened. `onEvent` gets `progress` and then
 * exactly one `done`, `cancelled` or `failed`. Rejects with a `VfsError` when it cannot start.
 */
export interface ChecksumClient {
	start(
		handle: ListingHandle,
		id: EntryId,
		algorithm: VerifyAlgorithm,
		onEvent: (event: ChecksumEvent) => void,
	): Promise<ChecksumRun>;
}
