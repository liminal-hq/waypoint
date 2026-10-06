// Names the Main window, for the window manager, after the folder of its active tab
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { useActiveTab } from '../tabs/TabsContext';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { locationLabel, useTabTitle } from '../tabs/tabTitle';
import { useWindowTitle } from '@liminal-hq/waypoint-chrome/WindowTitle/useWindowTitle';

/**
 * The OS title for a tab: its folder's name, and until Rust has resolved the location (when `useTabTitle`
 * stands in the whole display path) the last part of that path, so the title never flashes a full path.
 */
export function windowTitleFor(title: string, location: Pick<Location, 'display'>): string {
	return title === location.display ? locationLabel(location) : title;
}

function TabWindowTitle({ tab }: { tab: TabSnapshot }) {
	useWindowTitle(windowTitleFor(useTabTitle(tab), tab.location));
	return null;
}

/**
 * Renders nothing. The app switcher and the task bar show the folder name alone, as the tab does, so
 * several windows tell apart by the folder each has open; the app's own name comes from its icon. A
 * window with no tab leaves the title as it was.
 */
export function ActiveTabWindowTitle() {
	const tab = useActiveTab();
	return tab ? <TabWindowTitle tab={tab} /> : null;
}
