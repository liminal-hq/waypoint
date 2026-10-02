// The window key for the Shelf: Ctrl+B shows and hides it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';

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
 * What it does is `toggle`: show or hide the dock, or raise or hide the Shelf window.
 */
export function useShelfShortcuts(toggle: () => void): void {
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
			toggle();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [toggle]);
}
