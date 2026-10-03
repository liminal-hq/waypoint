// The frontend's view of facts about the running application, which About shows
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** Where the application's own details come from. */
export interface AppInfoClient {
	/** The version the application was built as ("0.1.0"). Rejects when it cannot be read. */
	getVersion(): Promise<string>;
}
