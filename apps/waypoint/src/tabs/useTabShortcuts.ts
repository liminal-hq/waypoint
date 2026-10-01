// The window-level tab keys from docs/interactions.md section 2 that need no groups or pairs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import { useActiveTab } from './TabsContext';
import { useTabActions, type TabActions } from './tabActions';

/**
 * Ctrl+T and Ctrl+W open and close, Ctrl+Tab and Ctrl+Shift+Tab step through the tabs in strip
 * order (most-recently-used order is a follow-up), and Alt+1 to Alt+9 go to that tab. Reopen
 * Closed Tab (Ctrl+Shift+T) needs a history of closed tabs in the session and is not here yet.
 */
export function handleTabKey(
	event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>,
	actions: TabActions,
	activeId: number | undefined,
): boolean {
	const key = event.key.toLowerCase();
	const modifier = event.ctrlKey || event.metaKey;
	if (modifier && !event.altKey && !event.shiftKey) {
		if (key === 't') {
			actions.newTab();
			return true;
		}
		if (key === 'w') {
			if (activeId !== undefined) actions.close(activeId);
			return true;
		}
	}
	if (event.ctrlKey && !event.altKey && key === 'tab') {
		actions.cycle(event.shiftKey ? -1 : 1);
		return true;
	}
	if (event.altKey && !modifier && !event.shiftKey && /^[1-9]$/.test(event.key)) {
		actions.goTo(Number(event.key));
		return true;
	}
	return false;
}

export function useTabShortcuts(): void {
	const actions = useTabActions();
	const activeId = useActiveTab()?.id;
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.isComposing) return;
			if (handleTabKey(event, actions, activeId)) event.preventDefault();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [actions, activeId]);
}
