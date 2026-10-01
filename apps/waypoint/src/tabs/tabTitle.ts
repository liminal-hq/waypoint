// A tab's title: the name of its folder, once Rust has resolved the location
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { useVfsClient } from '../browse/VfsClientContext';
import { useLocationInfo } from '../nav/locationInfo';

export function useTabTitle(tab: Pick<TabSnapshot, 'location'>): string {
	const info = useLocationInfo(useVfsClient(), tab.location);
	// Until Rust has answered, the full display path stands in for the folder name.
	return info?.segments[info.segments.length - 1]?.label ?? tab.location.display;
}

/** The last part of a location's display path, for places that cannot wait for the lookup (menus, announcements). */
export function locationLabel(location: Pick<Location, 'display'>): string {
	const parts = location.display.split(/[\\/]/).filter((part) => part !== '');
	return parts[parts.length - 1] ?? location.display;
}
