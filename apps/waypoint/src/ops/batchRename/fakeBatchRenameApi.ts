// A scriptable BatchRenameApi for tests: it answers each preview as the test says and records every call
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { BatchPreview } from '@liminal-hq/waypoint-protocol/generated/BatchPreview';
import type { JobId } from '@liminal-hq/waypoint-protocol/generated/JobId';
import type { JobRequest } from '@liminal-hq/waypoint-protocol/generated/JobRequest';
import type { PreviewRow } from '@liminal-hq/waypoint-protocol/generated/PreviewRow';
import type { BatchRenameApi } from './batchRenameApi';

/** A preview with these rows, its counts worked out the way the plugin does. */
export function previewOf(
	rows: Array<Partial<PreviewRow> & Pick<PreviewRow, 'from' | 'to'>>,
	extra: Partial<BatchPreview> = {},
): BatchPreview {
	const full: PreviewRow[] = rows.map((row, index) => ({
		index,
		changed: row.from !== row.to,
		extensionChanged: false,
		problems: [],
		...row,
	}));
	const blocking = full.filter((row) => row.problems.some((p) => p.kind !== 'unchangedSkip'));
	const ruleErrors = extra.ruleErrors ?? [];
	return {
		rows: full,
		ruleErrors,
		nowMs: 1_700_000_000_000,
		problems: blocking.length + ruleErrors.length,
		changes: full.filter((row) => row.changed).length,
		...extra,
	};
}

/**
 * Answers each preview with `respond(request)`, which a test replaces to make the next answer
 * whatever it needs (a rejection included). Nothing is derived here: the fake does not know what a
 * rule does.
 */
export class FakeBatchRenameApi implements BatchRenameApi {
	/** Every request `preview` was called with, in order. */
	readonly previews: JobRequest[] = [];
	/** Every request `apply` was called with, in order. */
	readonly applied: JobRequest[] = [];
	respond: (request: JobRequest) => BatchPreview | Promise<BatchPreview> = () => previewOf([]);
	applyResult: JobId | Error = 1;

	async preview(request: JobRequest): Promise<BatchPreview> {
		this.previews.push(request);
		return this.respond(request);
	}

	async apply(request: JobRequest): Promise<JobId> {
		this.applied.push(request);
		if (this.applyResult instanceof Error) throw this.applyResult;
		return this.applyResult;
	}
}
