// The window keys for the view: Ctrl+1 and Ctrl+2 choose Grid and List, Ctrl+H toggles hidden files
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import type { ViewStore } from './viewStore';

/** Ctrl+3 to Ctrl+5 (Columns, Compact, Disk usage) belong to views that do not exist yet. */
export function useViewShortcuts(store: ViewStore): void {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.isComposing) return;
			if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey) return;
			const state = store.getState();
			if (event.key === '1') state.setMode('grid');
			else if (event.key === '2') state.setMode('list');
			else if (event.key.toLowerCase() === 'h') state.toggleHidden();
			else return;
			event.preventDefault();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [store]);
}
