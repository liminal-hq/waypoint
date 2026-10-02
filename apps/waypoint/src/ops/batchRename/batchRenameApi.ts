// What the batch rename dialog needs from the operations plugin, behind an interface so it renders against a fake
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { BatchPreview } from '@liminal-hq/waypoint-protocol/generated/BatchPreview';
import type { JobId } from '@liminal-hq/waypoint-protocol/generated/JobId';
import type { JobRequest } from '@liminal-hq/waypoint-protocol/generated/JobRequest';
import type { Sources } from '@liminal-hq/waypoint-protocol/generated/Sources';

/**
 * The two calls a batch rename makes. Rust works out the new names and every clash (the frontend
 * never re-derives them): `preview` asks what the rules would do, and `apply` queues the job.
 * Both take the same `batchRename` request, and both reject with the plugin's `{ kind, message }`.
 */
export interface BatchRenameApi {
	preview(request: JobRequest): Promise<BatchPreview>;
	apply(request: JobRequest): Promise<JobId>;
}

/** What is being renamed, as the caller of `openBatchRename` knows it. */
export interface BatchRenameSelection {
	/** A selection handle and range, or explicit locations. */
	sources: Sources;
	/** How many entries are selected, for the dialog's description before the first preview. */
	count?: number;
}

/** The text of whatever a rejected command carried. */
export function errorText(error: unknown): string {
	if (typeof error === 'object' && error !== null && 'message' in error) {
		const message = (error as { message: unknown }).message;
		if (typeof message === 'string') return message;
	}
	return error instanceof Error ? error.message : String(error);
}
