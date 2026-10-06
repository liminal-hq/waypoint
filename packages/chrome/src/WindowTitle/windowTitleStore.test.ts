// Verifies the title store holds the latest title and tells only real changes to its listeners
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createWindowTitleStore } from './windowTitleStore';

describe('createWindowTitleStore', () => {
	it('starts empty, holds the latest title and notifies on a change only', () => {
		const store = createWindowTitleStore();
		const listener = vi.fn();
		const stop = store.subscribe(listener);
		expect(store.get()).toBeUndefined();
		store.set('One');
		store.set('One');
		expect(store.get()).toBe('One');
		expect(listener).toHaveBeenCalledTimes(1);
		stop();
		store.set('Two');
		expect(listener).toHaveBeenCalledTimes(1);
	});
});
