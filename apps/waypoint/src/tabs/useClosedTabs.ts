// The recently closed tabs for a menu: always read from a fresh snapshot, never the cached one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ClosedTab } from '@liminal-hq/waypoint-protocol/generated/ClosedTab';
import { useCallback, useState } from 'react';
import { useTabsApi } from './TabsContext';

/** How many closed tabs a menu lists (the store remembers the same number). */
export const RECENTLY_CLOSED_LIMIT = 10;

/**
 * Closing a tab emits no event for the closed list, so the store's cached snapshot can be out of
 * date. A menu calls `refresh()` before it opens and shows what that resolves to; `closed` holds
 * the latest result for anything rendered while the menu is up.
 */
export function useClosedTabs(): {
	closed: ClosedTab[];
	refresh: () => Promise<ClosedTab[]>;
} {
	const api = useTabsApi();
	const [closed, setClosed] = useState<ClosedTab[]>([]);
	const refresh = useCallback(async () => {
		const fresh = (await api.getSnapshot()).closed.slice(0, RECENTLY_CLOSED_LIMIT);
		setClosed(fresh);
		return fresh;
	}, [api]);
	return { closed, refresh };
}
