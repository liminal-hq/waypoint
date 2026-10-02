// Verifies the Shelf's connection to the command bridge: the facts follow the store and the actions do what Ctrl+B, the menu and the panel do
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ListingSession } from '../browse/useListingSession';
import { createCommandBridge } from '../commands/commandBridge';
import { FakeShelfWindowClient } from '../services/fakeShelfWindowClient';
import { fileLocation } from '../services/fakeVfsClient';
import type { ShelfActions } from './shelfActions';
import { createShelfPlacement } from './shelfPlacement';
import { createShelfStore } from './shelfStore';
import { useShelfCommands } from './useShelfCommands';

function setup(session: ListingSession | null = null) {
	const bridge = createCommandBridge();
	const store = createShelfStore();
	const actions = { addSelection: vi.fn(async () => 1) } as unknown as ShelfActions;
	const placement = createShelfPlacement({
		store,
		api: { setShelfUndocked: vi.fn(async () => {}) },
		client: new FakeShelfWindowClient(),
		inWindow: false,
		say: () => {},
		announce: () => {},
	});
	renderHook(() => useShelfCommands(bridge, store, actions, () => session, placement));
	return { bridge, store, actions };
}

describe('the facts', () => {
	it('follow whether the panel is open and how many items the Shelf holds', () => {
		const { bridge, store } = setup();
		expect(bridge.store.getState().facts).toMatchObject({ shelfOpen: false, shelfCount: 0 });
		act(() => store.getState().setOpen(true));
		act(() =>
			store.getState().sync(
				[
					{
						id: 1,
						location: fileLocation('/a/b'),
						name: 'b',
						addedMs: 1,
						origin: fileLocation('/a'),
					},
				],
				1,
			),
		);
		expect(bridge.store.getState().facts).toMatchObject({ shelfOpen: true, shelfCount: 1 });
	});
});

describe('the actions', () => {
	it('toggle the panel, and Focus Shelf opens it and asks for the focus', () => {
		const { bridge, store } = setup();
		const { actions } = bridge.store.getState();
		act(() => actions.toggleShelf());
		expect(store.getState().open).toBe(true);
		act(() => actions.toggleShelf());
		expect(store.getState().open).toBe(false);
		act(() => actions.focusShelf());
		expect(store.getState().open).toBe(true);
		expect(store.getState().focusRequests).toBe(1);
	});

	it('Add to Shelf puts the active pane’s selection on the Shelf, and does nothing with no pane', () => {
		const none = setup();
		none.bridge.store.getState().actions.addToShelf();
		expect(none.actions.addSelection).not.toHaveBeenCalled();
		const session = {} as ListingSession;
		const some = setup(session);
		some.bridge.store.getState().actions.addToShelf();
		expect(some.actions.addSelection).toHaveBeenCalledWith(session);
	});
});
