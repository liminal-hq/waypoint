// The window keys for panes: F3 toggles the split, F6 and Shift+F6 move between the panes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef } from 'react';
import { useTabsSnapshot } from './TabsContext';
import { pairOfTab } from './pairLayout';
import { usePairActions, type PairActions } from './pairActions';

type KeyEventLike = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>;

/**
 * F3 (no modifiers) toggles the split of the active tab. F6 focuses the next pane of the active
 * pair and Shift+F6 the previous; on a tab that is not paired F6 does nothing and is left alone.
 */
export function handlePairKey(
	event: KeyEventLike,
	actions: Pick<PairActions, 'toggleSplit' | 'focusPane'>,
	active: number | null,
	paired: boolean,
): boolean {
	if (event.ctrlKey || event.metaKey || event.altKey) return false;
	if (event.key === 'F3' && !event.shiftKey) {
		if (active === null) return false;
		actions.toggleSplit(active);
		return true;
	}
	if (event.key === 'F6' && paired) {
		actions.focusPane(event.shiftKey ? -1 : 1);
		return true;
	}
	return false;
}

export function usePairShortcuts(): void {
	const actions = usePairActions();
	const snapshot = useTabsSnapshot();
	const latest = useRef({ actions, snapshot });
	latest.current = { actions, snapshot };

	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.isComposing) return;
			const { actions: current, snapshot: state } = latest.current;
			const active = state?.active ?? null;
			if (
				handlePairKey(event, current, active, pairOfTab(state?.pairs ?? [], active) !== undefined)
			) {
				event.preventDefault();
			}
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, []);
}
