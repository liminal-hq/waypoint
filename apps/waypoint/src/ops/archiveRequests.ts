// The requests Extract and Compress make, and the words for what planning found
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ArchiveFormat } from '@liminal-hq/waypoint-protocol/generated/ArchiveFormat';
import type { ExtractLayout } from '@liminal-hq/waypoint-protocol/generated/ExtractLayout';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import type { PlanPreview } from '@liminal-hq/waypoint-protocol/generated/PlanPreview';
import type { Location, JobRequest } from '../services/opsClient';
import { opsErrorOf } from '../archives/askPassphrase';
import { tn } from '../i18n/messages';

const NO_OPTIONS = { conflict: null, verify: null } as const;

/**
 * Extracts `archives` (the archive files, or the top of an open archive) into `destination`, or
 * beside each archive when there is none. `allowLarge` answers an `archiveLimit` the person was
 * asked about.
 */
export function extractRequest(
	archives: readonly Location[],
	destination: Location | null,
	windowLabel: string,
	options: { layout?: ExtractLayout; allowLarge?: boolean } = {},
): JobRequest {
	return {
		kind: { kind: 'extract' },
		sources: { kind: 'locations', locations: [...archives] },
		destination,
		name: null,
		options: NO_OPTIONS,
		originWindow: windowLabel,
		archive: {
			kind: 'extract',
			layout: options.layout ?? 'auto',
			allowLarge: options.allowLarge ?? false,
		},
	};
}

/** Packs `sources` into one new archive called `name` (its extension already on) in `folder`. */
export function compressRequest(
	sources: readonly Location[],
	folder: Location,
	name: string,
	format: ArchiveFormat,
	windowLabel: string,
): JobRequest {
	return {
		kind: { kind: 'compress' },
		sources: { kind: 'locations', locations: [...sources] },
		destination: folder,
		name,
		options: NO_OPTIONS,
		originWindow: windowLabel,
		archive: { kind: 'compress', format },
	};
}

/** How many entries a plan left out for their names or links. */
export function leftOutCount(preview: Pick<PlanPreview, 'notes'>): number {
	return preview.notes.filter((note) => note.kind === 'leftOut').length;
}

/** The sentence for entries left out of an extraction. */
export function leftOutText(count: number): string {
	return tn('files.extract.leftOut', count);
}

/** The limit a failed plan was stopped by, or `null` when it failed for another reason. */
export function limitOf(failure: unknown): Extract<OpsError, { kind: 'archiveLimit' }> | null {
	const error = opsErrorOf(failure);
	return error?.kind === 'archiveLimit' ? error : null;
}
