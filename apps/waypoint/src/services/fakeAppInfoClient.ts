// An in-memory AppInfoClient for tests and the `?demo` window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AppInfoClient } from './appInfoClient';

/** Reports a version that a test chooses, or fails to read it when told to. */
export class FakeAppInfoClient implements AppInfoClient {
	/** How many times `getVersion()` was called. */
	reads = 0;

	constructor(
		private version = '0.0.0-test',
		private failure = false,
	) {}

	/** Makes `getVersion()` reject, as the real client does when the application cannot say. */
	failReads(): void {
		this.failure = true;
	}

	getVersion(): Promise<string> {
		this.reads += 1;
		return this.failure ? Promise.reject(new Error('no version')) : Promise.resolve(this.version);
	}
}
