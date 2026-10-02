// Connects the Shelf to the command bridge: the facts the registry reads and the actions its commands run
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import type { ListingSession } from '../browse/useListingSession';
import type { CommandBridge } from '../commands/commandBridge';
import type { ShelfActions } from './shelfActions';
import type { ShelfStore } from './shelfStore';

/**
 * The registry's Shelf commands (`toggleShelf`, `addToShelf`, `focusShelf`) are this window's:
 * the facts follow the store, and the actions are what Ctrl+B, the menu and the panel already do.
 * Add to Shelf acts on the selection of the pane the file commands act on (`activeSelection`).
 */
export function useShelfCommands(
	bridge: CommandBridge,
	store: ShelfStore,
	actions: ShelfActions,
	activeSession: () => ListingSession | null,
): void {
	useEffect(() => {
		const publish = () => {
			const { open, items } = store.getState();
			bridge.patchFacts({ shelfOpen: open, shelfCount: items.length });
		};
		publish();
		return store.subscribe(publish);
	}, [bridge, store]);

	useEffect(() => {
		bridge.patchActions({
			toggleShelf: () => store.getState().toggleOpen(),
			focusShelf: () => {
				store.getState().setOpen(true);
				store.getState().requestFocus();
			},
			addToShelf: () => {
				const session = activeSession();
				if (session) void actions.addSelection(session);
			},
		});
	}, [bridge, store, actions, activeSession]);
}
