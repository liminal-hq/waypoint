// The other windows a menu offers to move tabs into, read when it opens without holding the menu back
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { useEffect, useState } from 'react';
import type { WindowActions } from './windowActions';

/** How long the window list may take before the menu is left without it. */
export const OTHER_WINDOWS_TIMEOUT_MS = 1500;

/**
 * The other windows, read fresh each time the calling menu opens (they come and go unheard). It is
 * empty until the list arrives, and stays empty if the read fails or takes longer than
 * `OTHER_WINDOWS_TIMEOUT_MS`, so the menu can open at once with the items that need no windows.
 */
export function useOtherWindows(windows: Pick<WindowActions, 'otherWindows'>): WindowSummary[] {
	const [others, setOthers] = useState<WindowSummary[]>([]);
	useEffect(() => {
		let current = true;
		const timer = setTimeout(() => {
			current = false;
		}, OTHER_WINDOWS_TIMEOUT_MS);
		windows
			.otherWindows()
			.then((list) => {
				if (current) setOthers(list);
			})
			.catch(() => undefined)
			.finally(() => clearTimeout(timer));
		return () => {
			current = false;
			clearTimeout(timer);
		};
	}, [windows]);
	return others;
}
