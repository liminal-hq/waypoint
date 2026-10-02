// The window key for the Inspector: F11 shows and hides it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import type { InspectorStore } from './inspectorStore';

/** Whether a key event is F11 with nothing else held. */
export function isInspectorKey(
	event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>,
) {
	return (
		event.key === 'F11' && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey
	);
}

/**
 * F11 comes from the shortcut table in docs/interactions.md. A key a dialog or a menu already
 * took is left alone. Unlike a letter, F11 means the same in a text field, so a field does not
 * keep it.
 */
export function useInspectorShortcuts(store: InspectorStore): void {
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.isComposing || !isInspectorKey(event)) return;
			event.preventDefault();
			store.getState().toggleOpen();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [store]);
}
