// The window keys for the sidebar: F9 shows and hides it, Ctrl+D pins the current folder
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useEffect } from 'react';
import type { SidebarStore } from './sidebarStore';

/**
 * F9 and Ctrl+D come from the shortcut table in docs/interactions.md (Ctrl+B belongs to the Shelf).
 * `addFavourite` is called with the active tab's folder and does nothing without one.
 */
export function useSidebarShortcuts(
	store: SidebarStore,
	current: Location | undefined,
	addFavourite: (location: Location) => void,
): void {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.isComposing) return;
			// Ctrl+D in the path bar or a rename field is the field's, not a request to pin the folder.
			if (isTextField(event.target)) return;
			if (
				event.key === 'F9' &&
				!event.ctrlKey &&
				!event.metaKey &&
				!event.altKey &&
				!event.shiftKey
			) {
				event.preventDefault();
				store.getState().toggleOpen();
			} else if (
				event.key.toLowerCase() === 'd' &&
				(event.ctrlKey || event.metaKey) &&
				!event.altKey &&
				!event.shiftKey
			) {
				// The webview would otherwise offer its own bookmark.
				event.preventDefault();
				if (current) addFavourite(current);
			}
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [store, current, addFavourite]);
}

function isTextField(target: EventTarget | null): boolean {
	return (
		target instanceof HTMLElement &&
		(target.isContentEditable || target.closest('input, textarea, select') !== null)
	);
}
