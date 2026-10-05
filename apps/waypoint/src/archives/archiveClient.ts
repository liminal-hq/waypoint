// What the page needs from the archive provider beyond the file system: giving it a password, and hearing that a listing will be slow
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { Unsubscribe } from '../services/vfsClient';

/** A listing of an archive that has to read the whole file to be made. */
export interface SlowListing {
	/** The archive file that is being read. */
	container: Location;
	bytes: number;
	/** The format as people name it: `tar.gz`. */
	format: string;
}

/**
 * The seam between the page and Rust's archive provider, injected so every view is tested without
 * it. The password is sent once and Rust keeps it in memory only; it is never part of a location.
 */
export interface ArchiveClient {
	/** Gives Rust the password of the archive file `location` is, or is in. Rejects with a `VfsError`. */
	unlock(location: Location, passphrase: string): Promise<void>;
	/** Hears that a listing will take a while, as it starts. */
	onSlowListing(listener: (listing: SlowListing) => void): Unsubscribe;
}
