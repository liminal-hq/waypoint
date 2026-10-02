// Connects the Shelf to the command bridge: the facts the registry reads and the actions its commands run
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import type { ListingSession } from '../browse/useListingSession';
import type { CommandBridge } from '../commands/commandBridge';
import type { ShelfActions } from './shelfActions';
import type { ShelfPlacement } from './shelfPlacement';
import type { ShelfStore } from './shelfStore';

/**
 * The registry's Shelf commands (`toggleShelf`, `addToShelf`, `focusShelf`) are this window's:
 * the facts follow the store, and the actions are what Ctrl+B, the menu and the panel already do.
 * Add to Shelf acts on the selection of the pane the file commands act on (`activeSelection`).
 * Toggling and focusing follow where the Shelf is (`placement`): the dock, or the Shelf window.
 */
export function useShelfCommands(
	bridge: CommandBridge,
	store: ShelfStore,
	actions: ShelfActions,
	activeSession: () => ListingSession | null,
	placement: ShelfPlacement,
): void {
	useEffect(() => {
		const publish = () => {
			const { open, items, undocked, windowShown } = store.getState();
			// Undocked, the Shelf is "open" while its window is on screen.
			bridge.patchFacts({
				shelfOpen: undocked ? windowShown : open,
				shelfUndocked: undocked,
				shelfCount: items.length,
			});
		};
		publish();
		return store.subscribe(publish);
	}, [bridge, store]);

	useEffect(() => {
		bridge.patchActions({
			toggleShelf: placement.toggle,
			focusShelf: placement.focus,
			undockShelf: () => void placement.undock(),
			dockShelf: () => void placement.dock(),
			addToShelf: () => {
				const session = activeSession();
				if (session) void actions.addSelection(session);
			},
		});
	}, [bridge, actions, activeSession, placement]);
}
