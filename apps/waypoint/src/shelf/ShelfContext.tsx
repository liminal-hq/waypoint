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
import type { ShelfWindowClient } from '../services/shelfWindowClient';
import { createTauriShelfWindowClient } from '../services/tauriShelfWindowClient';
import { announce } from '../tabs/announcer';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { createShelfActions, type ShelfActions } from './shelfActions';
import { createShelfPlacement, type ShelfPlacement } from './shelfPlacement';
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

const ShelfPlacementContext = createContext<ShelfPlacement | null>(null);

/** Where the Shelf is shown and the moves between the dock and its window, or `null` outside a provider. */
export function useShelfPlacement(): ShelfPlacement | null {
	return useContext(ShelfPlacementContext);
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
 *
 * Where the Shelf lives is the session's too (`snapshot.shelfWindow`): undocked, it is the Shelf
 * window's, every main window keeps no dock, and the toggles act on that window. The Shelf window
 * mounts this provider as well (`inWindow`), so it follows the same items, groups and selection
 * rules; its panel is always open.
 */
export function ShelfProvider({
	activeSession,
	inWindow = false,
	windowClient,
	children,
}: {
	/** The listing the Add to Shelf command acts on: the active pane's. */
	activeSession: () => ListingSession | null;
	/** This is the Shelf window, not a main window with a dock. */
	inWindow?: boolean;
	/** What raises and hides the Shelf window; the real one unless a test supplies its own. */
	windowClient?: ShelfWindowClient;
	children: ReactNode;
}) {
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const vfs = useVfsClient();
	const clipboard = useClipboardService();
	const bridge = useCommandBridge();
	const [store] = useState<ShelfStore>(() =>
		createShelfStore(inWindow ? { open: true, undocked: true } : {}),
	);
	const [client] = useState<ShelfWindowClient>(
		() => windowClient ?? createTauriShelfWindowClient(),
	);

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

	const placement = useMemo(
		() =>
			createShelfPlacement({
				store,
				api,
				client,
				inWindow,
				say: (text) => void showNotice(text),
				announce,
			}),
		[store, api, client, inWindow],
	);

	// The session's Shelf, as of its revision; an older one never replaces a newer.
	useEffect(() => {
		if (!snapshot) return;
		store.getState().sync(snapshot.shelf, snapshot.revision);
		store.getState().setWindow(snapshot.shelfWindow);
	}, [store, snapshot]);

	// Docking again (by the button, or by closing the Shelf window) brings the dock back in this window.
	const undocked = useStore(store, (state) => state.undocked);
	const wasUndocked = useRef(undocked);
	useEffect(() => {
		if (wasUndocked.current && !undocked && !inWindow) store.getState().setOpen(true);
		wasUndocked.current = undocked;
	}, [store, undocked, inWindow]);

	// While the Shelf is its own window, follow whether it is on screen (subscribe first, then read).
	useEffect(() => {
		if (!undocked) return;
		let active = true;
		let changed = false;
		store.getState().setWindowShown(true);
		const stop = client.onVisibleChange((shown) => {
			if (!active) return;
			changed = true;
			store.getState().setWindowShown(shown);
		});
		client.visible().then(
			(shown) => {
				if (active && !changed) store.getState().setWindowShown(shown);
			},
			() => {},
		);
		return () => {
			active = false;
			stop();
		};
	}, [store, client, undocked]);

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

	useShelfShortcuts(placement.toggle);
	useShelfCommands(bridge, store, actions, () => latest.current.activeSession(), placement);

	return (
		<ShelfStoreContext.Provider value={store}>
			<ShelfPlacementContext.Provider value={placement}>
				<ShelfActionsContext.Provider value={actions}>{children}</ShelfActionsContext.Provider>
			</ShelfPlacementContext.Provider>
		</ShelfStoreContext.Provider>
	);
}
