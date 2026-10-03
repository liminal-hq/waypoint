// Verifies the accelerator rule the fake settings client mirrors from Rust
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { DEFAULT_ACCELERATOR, isAccelerator } from './accelerator';

describe('isAccelerator', () => {
	it('accepts the default and other modifier-and-key forms', () => {
		for (const good of [
			DEFAULT_ACCELERATOR,
			'ctrl+alt+w',
			'Super+Space',
			'CmdOrCtrl+Shift+F5',
			'Ctrl + Alt + K',
			'Ctrl+F24',
		]) {
			expect(isAccelerator(good), good).toBe(true);
		}
	});

	it('refuses a bare key, unknown or doubled words and empty or control text', () => {
		for (const bad of [
			'',
			'  ',
			'W',
			'Ctrl',
			'Ctrl+Nope',
			'Ctrl+A+B',
			'Ctrl+F25',
			'Ctrl+F0',
			'Ctrl+Control+W',
			'Ctrl++W',
			'Ctrl+Alt+W\n',
			`Ctrl+${'a'.repeat(64)}`,
		]) {
			expect(isAccelerator(bad), JSON.stringify(bad)).toBe(false);
		}
	});
});
