// Supplies a window's remembered folder views to whatever applies and writes them: the workspace and its menus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import type { FolderViewsClient } from '../services/folderViewsClient';
import { createFolderViewsStore, type FolderViewsHandle } from './folderViewStore';

const FolderViewsContext = createContext<FolderViewsHandle | null>(null);

interface FolderViewsProviderProps {
	client: FolderViewsClient | undefined;
	children: ReactNode;
}

/**
 * Follows `client`'s remembered folder views for as long as it is mounted. Without a client
 * (the in-memory demo, a window that cannot reach the plugin) no folder remembers a view and
 * every folder shows the window's.
 */
export function FolderViewsProvider({ client, children }: FolderViewsProviderProps) {
	const [handle, setHandle] = useState<FolderViewsHandle | null>(null);
	useEffect(() => {
		if (!client) {
			setHandle(null);
			return;
		}
		const created = createFolderViewsStore(client);
		setHandle(created);
		return () => created.dispose();
	}, [client]);
	return <FolderViewsContext.Provider value={handle}>{children}</FolderViewsContext.Provider>;
}

/** The window's folder views handle, or `null` where there is no service and before the store exists. */
export function useFolderViews(): FolderViewsHandle | null {
	return useContext(FolderViewsContext);
}
