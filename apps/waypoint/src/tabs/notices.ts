// A one-way feed of short messages for the status bar, for code that has no route to it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

type Listener = (text: string) => void;

const listeners = new Set<Listener>();

/** Shows `text` in the status bar of this window, if a window is listening. */
export function postNotice(text: string): void {
	for (const listener of [...listeners]) listener(text);
}

/** Calls `listener` with every notice until the returned function is called. */
export function onNotice(listener: Listener): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}
