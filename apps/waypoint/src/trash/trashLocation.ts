// Telling the Trash from a folder, for the few places that word things differently
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';

/** The Trash's own location, which the sidebar's Trash place opens. */
export const TRASH_LOCATION: Location = { display: 'Trash', uri: 'trash:/' };

/** Whether `location` is in the Trash. Rust decides everything else about it; this only picks the words for a failed listing. */
export function isTrashLocation(location: Location | undefined): boolean {
	return location !== undefined && /^trash:/i.test(location.uri);
}
