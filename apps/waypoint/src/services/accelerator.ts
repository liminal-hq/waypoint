// The global shortcut's accelerator rule, mirrored from `waypoint-settings` for the fake client
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** Used while no accelerator is set; the shortcut itself stays off until it is enabled (D118). */
export const DEFAULT_ACCELERATOR = 'Ctrl+Alt+W';

const MODIFIERS = [
	'ctrl',
	'control',
	'alt',
	'option',
	'shift',
	'super',
	'cmdorctrl',
	'commandorcontrol',
];
const NAMED_KEYS = [
	'space',
	'enter',
	'tab',
	'escape',
	'backspace',
	'delete',
	'insert',
	'home',
	'end',
	'pageup',
	'pagedown',
	'up',
	'down',
	'left',
	'right',
];

function group(word: string): string {
	if (['ctrl', 'control', 'cmdorctrl', 'commandorcontrol'].includes(word)) return 'ctrl';
	if (['alt', 'option'].includes(word)) return 'alt';
	return word;
}

function isKey(word: string): boolean {
	if (/^[a-z0-9]$/.test(word)) return true;
	const fn = /^f([1-9][0-9]?)$/.exec(word);
	if (fn) return Number(fn[1]) <= 24;
	return NAMED_KEYS.includes(word);
}

/**
 * Whether `text` is an accelerator, by the same rule Rust applies (`validate_accelerator`): one or
 * more distinct modifiers and exactly one key joined by `+`. The real client never calls this: Rust
 * is the one validator, and the page shows its answer.
 */
export function isAccelerator(text: string): boolean {
	if (text.trim() === '' || text.length > 64 || /[\u0000-\u001f\u007f-\u009f]/.test(text)) {
		return false;
	}
	const words = text
		.toLowerCase()
		.split('+')
		.map((word) => word.trim());
	if (words.some((word) => word === '')) return false;
	const key = words[words.length - 1] as string;
	const modifiers = words.slice(0, -1);
	if (
		modifiers.length === 0 ||
		!isKey(key) ||
		!modifiers.every((word) => MODIFIERS.includes(word))
	) {
		return false;
	}
	const groups = modifiers.map(group);
	return new Set(groups).size === groups.length;
}
