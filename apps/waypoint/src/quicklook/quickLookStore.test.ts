// Verifies the Quick Look store: it opens only where a host will show it, and closes when the host goes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { ListingSession } from '../browse/useListingSession';
import { createQuickLookStore } from './quickLookStore';

const request = { session: {} as ListingSession, move: () => null, onOpen: undefined };

describe('the Quick Look store', () => {
	it('refuses to open with no host, so a view never claims a Space nothing will answer', () => {
		const store = createQuickLookStore();
		expect(store.getState().open(request)).toBe(false);
		expect(store.getState().request).toBeNull();
	});

	it('opens and closes with a host attached, and closes when the host detaches', () => {
		const store = createQuickLookStore();
		const detach = store.getState().attach();
		expect(store.getState().open(request)).toBe(true);
		expect(store.getState().request).toBe(request);
		store.getState().close();
		expect(store.getState().request).toBeNull();
		store.getState().open(request);
		detach();
		expect(store.getState().request).toBeNull();
		expect(store.getState().hosts).toBe(0);
	});
});
