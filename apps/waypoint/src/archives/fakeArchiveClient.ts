// An in-memory archive client for tests: passwords it was given, and notices a test sends
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ArchiveClient, SlowListing } from './archiveClient';

export class FakeArchiveClient implements ArchiveClient {
	/** Every password given, with the location it was given for, in order. */
	readonly unlocked: Array<{ location: Location; passphrase: string }> = [];
	/** A refusal the next `unlock` rejects with, once. */
	failNext: unknown = null;
	private readonly listeners = new Set<(listing: SlowListing) => void>();

	unlock(location: Location, passphrase: string): Promise<void> {
		if (this.failNext !== null) {
			const error = this.failNext;
			this.failNext = null;
			return Promise.reject(error);
		}
		this.unlocked.push({ location, passphrase });
		return Promise.resolve();
	}

	onSlowListing(listener: (listing: SlowListing) => void) {
		this.listeners.add(listener);
		return () => void this.listeners.delete(listener);
	}

	/** Sends a notice as Rust would. */
	slowListing(listing: SlowListing): void {
		for (const listener of this.listeners) listener(listing);
	}
}
