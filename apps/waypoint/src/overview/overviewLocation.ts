// Telling Overview from a folder: its location, and the check the tab area and the listing manager make
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';

/** Overview's own location, which the sidebar's Overview place and the palette command open. */
export const OVERVIEW_LOCATION: Location = { display: 'Overview', uri: 'overview:/' };

/**
 * Whether `location` is Overview. It is a view and not a folder (`waypoint_vfs::overview_location`
 * in Rust), so no listing is opened for it and the tab shows the Overview page instead.
 */
export function isOverviewLocation(location: Pick<Location, 'uri'> | undefined): boolean {
	return location !== undefined && /^overview:\/*$/i.test(location.uri.trim());
}
