// Names the Main window, for the window manager, after the folder of its active tab
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { useActiveTab } from '../tabs/TabsContext';
import { useTabTitle } from '../tabs/tabTitle';
import { useWindowTitle } from './useWindowTitle';

function TabWindowTitle({ tab }: { tab: TabSnapshot }) {
	useWindowTitle(useTabTitle(tab));
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
