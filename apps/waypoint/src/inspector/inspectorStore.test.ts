// Verifies the Inspector's window memory: open, tab and a clamped width
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	clampWidth,
	createInspectorStore,
	DEFAULT_WIDTH,
	MAX_WIDTH,
	MIN_WIDTH,
} from './inspectorStore';

describe('the Inspector store', () => {
	it('starts closed on the Preview tab at its default width', () => {
		expect(createInspectorStore().getState()).toMatchObject({
			open: false,
			tab: 'preview',
			width: DEFAULT_WIDTH,
		});
	});

	it('toggles, and reopens on the tab it was last on', () => {
		const store = createInspectorStore();
		store.getState().toggleOpen();
		store.getState().setTab('properties');
		store.getState().toggleOpen();
		expect(store.getState().open).toBe(false);
		store.getState().toggleOpen();
		expect(store.getState()).toMatchObject({ open: true, tab: 'properties' });
	});

	it('opens on the Properties tab for right-click ▸ Properties, from closed or from Preview', () => {
		const store = createInspectorStore({ open: true, tab: 'preview' });
		store.getState().showProperties();
		expect(store.getState()).toMatchObject({ open: true, tab: 'properties' });
		const closed = createInspectorStore();
		closed.getState().showProperties();
		expect(closed.getState()).toMatchObject({ open: true, tab: 'properties' });
	});

	it('keeps the width between its least and most', () => {
		const store = createInspectorStore();
		store.getState().setWidth(10);
		expect(store.getState().width).toBe(MIN_WIDTH);
		store.getState().setWidth(5000);
		expect(store.getState().width).toBe(MAX_WIDTH);
		store.getState().setWidth(401.6);
		expect(store.getState().width).toBe(402);
		expect(clampWidth(DEFAULT_WIDTH)).toBe(DEFAULT_WIDTH);
	});
});
