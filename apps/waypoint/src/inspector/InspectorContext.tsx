// Gives the window its Inspector: the store, F11, and the facts and actions the command registry reads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, type ReactNode } from 'react';
import { useCommandBridge, type CommandBridge } from '../commands/commandBridge';
import { InspectorStoreContext, type InspectorStore } from './inspectorStore';
import { useInspectorShortcuts } from './useInspectorShortcuts';

/**
 * One Inspector per window. The store is the workspace's (its right-click menus open the panel
 * too), so this only wires it in: F11, and the registry's `toggleInspector` and `showProperties`
 * commands with the one fact, whether the panel is open.
 */
export function InspectorProvider({
	store,
	children,
}: {
	store: InspectorStore;
	children: ReactNode;
}) {
	const bridge = useCommandBridge();
	useInspectorShortcuts(store);
	useInspectorCommands(bridge, store);
	return <InspectorStoreContext.Provider value={store}>{children}</InspectorStoreContext.Provider>;
}

/** Publishes the Inspector's fact and actions to the command bridge. */
export function useInspectorCommands(bridge: CommandBridge, store: InspectorStore): void {
	useEffect(() => {
		const publish = () => bridge.patchFacts({ inspectorOpen: store.getState().open });
		publish();
		return store.subscribe(publish);
	}, [bridge, store]);
	useEffect(() => {
		bridge.patchActions({
			toggleInspector: () => store.getState().toggleOpen(),
			showProperties: () => store.getState().showProperties(),
		});
	}, [bridge, store]);
}
