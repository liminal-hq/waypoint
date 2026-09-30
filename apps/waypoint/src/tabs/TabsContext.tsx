// Supplies the window's `TabsApi` and its tab snapshot to everything below
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import type { TabsApi } from '../services/tabsApi';
import { createTabsStore, type TabsHandle } from './tabsStore';

interface TabsContextValue {
	api: TabsApi;
	handle: TabsHandle;
}

const TabsContext = createContext<TabsContextValue | null>(null);

interface TabsProviderProps {
	api: TabsApi;
	children: ReactNode;
}

/** Follows `api`'s session for as long as it is mounted; renders nothing until the first snapshot. */
export function TabsProvider({ api, children }: TabsProviderProps) {
	const [value, setValue] = useState<TabsContextValue | null>(null);
	useEffect(() => {
		const handle = createTabsStore(api);
		setValue({ api, handle });
		return () => handle.dispose();
	}, [api]);
	const ready = value?.api === api ? value : null;
	return ready ? <TabsContext.Provider value={ready}>{children}</TabsContext.Provider> : null;
}

function useTabsContext(): TabsContextValue {
	const value = useContext(TabsContext);
	if (!value) throw new Error('Tabs hooks must be used inside a TabsProvider');
	return value;
}

export function useTabsApi(): TabsApi {
	return useTabsContext().api;
}

/** The session snapshot, or `null` before the first one arrives. */
export function useTabsSnapshot(): SessionSnapshot | null {
	return useStore(useTabsContext().handle.store, (state) => state.snapshot);
}

/** The active tab, or `undefined` when the window has none. */
export function useActiveTab(): TabSnapshot | undefined {
	const snapshot = useTabsSnapshot();
	return snapshot?.tabs.find((tab) => tab.id === snapshot.active);
}
