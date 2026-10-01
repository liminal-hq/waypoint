// The real BatchRenameApi: the operations plugin's preview and submit commands through its guest-js
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as ops from '@liminal-hq/waypoint-plugin-ops';
import type { BatchRenameApi } from './batchRenameApi';

/** A `BatchRenameApi` over the `waypoint-ops` plugin. */
export function createTauriBatchRenameApi(): BatchRenameApi {
	return {
		preview: (request) => ops.previewBatchRename(request),
		apply: (request) => ops.submit(request),
	};
}
