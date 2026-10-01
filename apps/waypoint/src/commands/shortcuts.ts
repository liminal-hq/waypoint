// Reads a displayed shortcut ("Ctrl+Shift+Z") back into the key event that presses it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export interface ParsedShortcut {
	/** The key as `KeyboardEvent.key` reports it (letters in lower case, `Delete`, `F7`, `,`). */
	key: string;
	ctrl: boolean;
	shift: boolean;
	alt: boolean;
}

/** Splits "Ctrl+Shift+Z" into its modifiers and key. A plain `+` key is not used by any command. */
export function parseShortcut(shortcut: string): ParsedShortcut {
	const parts = shortcut.split('+');
	const key = parts.pop() ?? '';
	return {
		key: key.length === 1 ? key.toLowerCase() : key,
		ctrl: parts.includes('Ctrl'),
		shift: parts.includes('Shift'),
		alt: parts.includes('Alt'),
	};
}

/** The `KeyboardEvent` init that presses `shortcut`, with the key upper-cased under Shift as a keyboard reports it. */
export function keyEventInit(shortcut: string): KeyboardEventInit {
	const parsed = parseShortcut(shortcut);
	return {
		key: parsed.shift && parsed.key.length === 1 ? parsed.key.toUpperCase() : parsed.key,
		ctrlKey: parsed.ctrl,
		shiftKey: parsed.shift,
		altKey: parsed.alt,
		bubbles: true,
		cancelable: true,
	};
}
