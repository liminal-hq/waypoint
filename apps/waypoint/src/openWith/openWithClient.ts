// What Open With needs from the mime-apps plugin: which features work here, the applications for some locations, and starting one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Handlers, PluginStatus } from '@liminal-hq/plugin-mime-apps';

/**
 * The seam between Open With and the plugin behind it, injected so the menu and the chooser are
 * tested without one. The plugin owns the associations (A63); this only asks and starts.
 *
 * Every method rejects with a `MimeAppsError` (see `isMimeAppsError`) when the plugin refuses.
 */
export interface OpenWithClient {
	/** Which features work on this system, each with a reason when it does not. */
	getStatus(): Promise<PluginStatus>;
	/** The default, recommended and other applications for the locations' type. */
	handlers(uris: string[]): Promise<Handlers>;
	/** Opens the locations in the application with this id. */
	openWith(uris: string[], appId: string): Promise<void>;
	/** Opens each location in its default application. */
	openDefault(uris: string[]): Promise<void>;
	/** The system's own chooser, as a child of this window; rejects with `{ kind: 'cancelled' }` when dismissed. */
	choose(uris: string[]): Promise<void>;
	/** The address of an application's icon, for an `<img src>`. */
	iconUrl(appId: string, size?: number): string;
}
