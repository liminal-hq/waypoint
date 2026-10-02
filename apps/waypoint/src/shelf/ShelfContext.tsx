// Gives the window its Shelf: the store the session feeds, the actions over it, the key, the commands and the lazy check for missing files
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	createContext,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
	type ReactNode,
} from 'react';
import { useStore } from 'zustand';
import { showNotice } from '../app/notices';
import type { ListingSession } from '../browse/useListingSession';
import { useVfsClient } from '../browse/VfsClientContext';
import { useCommandBridge } from '../commands/commandBridge';
import { useClipboardService } from '../ops/ClipboardContext';
import { announce } from '../tabs/announcer';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { createShelfActions, type ShelfActions } from './shelfActions';
import { createShelfStore, ShelfStoreContext, type ShelfStore } from './shelfStore';
import { useShelfCommands } from './useShelfCommands';
import { useShelfShortcuts } from './useShelfShortcuts';

/** The least time between two looks at the files when the window is focused again, so switching back and forth does not keep asking. */
export const RECHECK_MS = 10_000;

const ShelfActionsContext = createContext<ShelfActions | null>(null);

/** The Shelf's actions, or `null` outside a provider (a view on its own, with no window around it). */
export function useShelfActions(): ShelfActions | null {
	return useContext(ShelfActionsContext);
}

/** The Shelf's store, or `null` outside a provider. */
export function useOptionalShelfStore(): ShelfStore | null {
	return useContext(ShelfStoreContext);
}

/**
 * One Shelf per window. Its items are the session's (every window's `shelfChanged` arrives through
 * the tabs snapshot and is taken here with its revision), and whether the panel is open and how
 * wide is this window's own. The files are looked at only while the panel is open: when it opens,
 * when items arrive, and when the window is focused again (at most every `RECHECK_MS`).
 */
export function ShelfProvider({
	activeSession,
	children,
}: {
	/** The listing the Add to Shelf command acts on: the active pane's. */
	activeSession: () => ListingSession | null;
	children: ReactNode;
}) {
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const vfs = useVfsClient();
	const clipboard = useClipboardService();
	const bridge = useCommandBridge();
	const [store] = useState<ShelfStore>(() => createShelfStore());

	const latest = useRef({ snapshot, clipboard, activeSession });
	latest.current = { snapshot, clipboard, activeSession };
	const actions = useMemo(
		() =>
			createShelfActions({
				store,
				api,
				vfs,
				say: (text) => void showNotice(text),
				announce,
				activeTab: () => latest.current.snapshot?.active ?? null,
				clipboard: () => latest.current.clipboard,
				writeText: (text) => navigator.clipboard.writeText(text),
			}),
		[store, api, vfs],
	);

	// The session's Shelf, as of its revision; an older one never replaces a newer.
	useEffect(() => {
		if (snapshot) store.getState().sync(snapshot.shelf, snapshot.revision);
	}, [store, snapshot]);

	// Look at the files while the panel shows: now, when the Shelf changes, and when the window returns.
	const open = useStore(store, (state) => state.open);
	const items = useStore(store, (state) => state.items);
	useEffect(() => {
		if (open) void actions.check(items);
	}, [open, items, actions]);
	useEffect(() => {
		if (!open) return;
		let last = Date.now();
		const onFocus = () => {
			if (Date.now() - last < RECHECK_MS) return;
			last = Date.now();
			void actions.check(store.getState().items, { force: true });
		};
		window.addEventListener('focus', onFocus);
		return () => window.removeEventListener('focus', onFocus);
	}, [open, actions, store]);

	useShelfShortcuts(store);
	useShelfCommands(bridge, store, actions, () => latest.current.activeSession());

	return (
		<ShelfStoreContext.Provider value={store}>
			<ShelfActionsContext.Provider value={actions}>{children}</ShelfActionsContext.Provider>
		</ShelfStoreContext.Provider>
	);
}
