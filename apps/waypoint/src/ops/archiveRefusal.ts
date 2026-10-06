// The words for why an archive cannot be changed: one sentence for each reason the planner gives
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ArchiveWriteRefusal } from '@liminal-hq/waypoint-protocol/generated/ArchiveWriteRefusal';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { tf, type MessageId } from '../i18n/messages';

/** The last part of an address or a path. */
function lastName(display: string): string {
	const parts = display.split(/[\\/]/).filter(Boolean);
	return parts[parts.length - 1] ?? display;
}

/** Why the archive file at `location` cannot be rewritten, in words a person can act on (D170). */
export function archiveRefusalText(location: Location, reason: ArchiveWriteRefusal): string {
	const name = lastName(location.display);
	if (reason.kind === 'readOnlyFormat') {
		return tf('ops.error.archiveNotWritable.readOnlyFormat', { name, format: reason.format });
	}
	return tf(`ops.error.archiveNotWritable.${reason.kind}` as MessageId, { name });
}
