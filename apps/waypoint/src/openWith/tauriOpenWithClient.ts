// The real OpenWithClient: the mime-apps plugin, with the chooser parented to this window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as plugin from '@liminal-hq/plugin-mime-apps';
import { currentWindowLabel } from '../ops/windowLabel';
import type { OpenWithClient } from './openWithClient';

/** An `OpenWithClient` over the `mime-apps` plugin. Create one per window. */
export function createTauriOpenWithClient(): OpenWithClient {
	return {
		getStatus: () => plugin.getStatus(),
		handlers: (uris) => plugin.handlers(uris),
		openWith: (uris, appId) => plugin.openWith(uris, appId),
		openDefault: (uris) => plugin.openDefault(uris),
		choose: (uris) => plugin.choose(uris, currentWindowLabel()),
		iconUrl: (appId, size) => plugin.appIconUrl(appId, size),
	};
}
