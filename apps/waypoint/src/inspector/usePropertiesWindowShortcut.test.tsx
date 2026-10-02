// Verifies Alt+Enter is taken only as Alt+Enter, only when the command ran, and not when something else took it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { isPropertiesWindowKey, usePropertiesWindowShortcut } from './usePropertiesWindowShortcut';

afterEach(cleanup);

const key = (init: KeyboardEventInit) => ({
	key: 'Enter',
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	shiftKey: false,
	isComposing: false,
	...init,
});

describe('isPropertiesWindowKey', () => {
	it('is Alt+Enter and nothing else', () => {
		expect(isPropertiesWindowKey(key({ altKey: true }))).toBe(true);
		expect(isPropertiesWindowKey(key({}))).toBe(false);
		expect(isPropertiesWindowKey(key({ altKey: true, ctrlKey: true }))).toBe(false);
		expect(isPropertiesWindowKey(key({ altKey: true, shiftKey: true }))).toBe(false);
		expect(isPropertiesWindowKey(key({ altKey: true, isComposing: true }))).toBe(false);
		expect(isPropertiesWindowKey(key({ altKey: true, key: 'a' }))).toBe(false);
	});
});

describe('usePropertiesWindowShortcut', () => {
	it('takes the key when the command ran', () => {
		const open = vi.fn().mockReturnValue(true);
		renderHook(() => usePropertiesWindowShortcut(open));
		const taken = fireEvent.keyDown(window, { key: 'Enter', altKey: true });
		expect(open).toHaveBeenCalledTimes(1);
		expect(taken).toBe(false);
	});

	it('leaves the key alone when the command is not offered', () => {
		const open = vi.fn().mockReturnValue(false);
		renderHook(() => usePropertiesWindowShortcut(open));
		expect(fireEvent.keyDown(window, { key: 'Enter', altKey: true })).toBe(true);
	});

	it('ignores a key something else took, and other keys', () => {
		const open = vi.fn().mockReturnValue(true);
		renderHook(() => usePropertiesWindowShortcut(open));
		const early = (event: Event) => event.preventDefault();
		window.addEventListener('keydown', early, true);
		fireEvent.keyDown(window, { key: 'Enter', altKey: true });
		window.removeEventListener('keydown', early, true);
		fireEvent.keyDown(window, { key: 'Enter' });
		expect(open).not.toHaveBeenCalled();
	});
});
