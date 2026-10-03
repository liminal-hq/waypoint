// The window key for the Properties window: Alt+Enter
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';

/** Whether a key event is Alt+Enter with nothing else held. */
export function isPropertiesWindowKey(
	event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey' | 'isComposing'>,
): boolean {
	return (
		event.key === 'Enter' &&
		event.altKey &&
		!event.ctrlKey &&
		!event.metaKey &&
		!event.shiftKey &&
		!event.isComposing
	);
}

/**
 * Alt+Enter comes from the shortcut table in docs/interactions.md. `open` says whether it did
 * something (the command may not be offered: in the Trash, or with several items selected), and
 * the key is taken only then. A key a dialog or a menu already took is left alone.
 */
export function usePropertiesWindowShortcut(open: () => boolean): void {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || !isPropertiesWindowKey(event)) return;
			if (open()) event.preventDefault();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [open]);
}
