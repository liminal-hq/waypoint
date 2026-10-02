// The window key for the Shelf: Ctrl+B shows and hides it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import type { ShelfStore } from './shelfStore';

/** Whether a key event is Ctrl+B (or Cmd+B) with nothing else held. */
export function isShelfKey(
	event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>,
) {
	return (
		event.key.toLowerCase() === 'b' &&
		(event.ctrlKey || event.metaKey) &&
		!event.altKey &&
		!event.shiftKey
	);
}

/**
 * Ctrl+B comes from the shortcut table in docs/interactions.md. A text field keeps it (the path
 * bar and a rename field are the field's), and a key a dialog or a menu already took is left alone.
 */
export function useShelfShortcuts(store: ShelfStore): void {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.isComposing || !isShelfKey(event)) return;
			const target = event.target;
			if (
				target instanceof HTMLElement &&
				(target.isContentEditable || target.closest('input, textarea, select') !== null)
			) {
				return;
			}
			event.preventDefault();
			store.getState().toggleOpen();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [store]);
}
