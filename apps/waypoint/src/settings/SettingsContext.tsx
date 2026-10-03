// Supplies a window's settings store to whatever reads them: the browser, the drag engine, the Settings pages
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import { createStore } from 'zustand/vanilla';
import { DEFAULT_SETTINGS, type Settings, type SettingsClient } from '../services/settingsClient';
import { createSettingsStore, type SettingsHandle, type SettingsState } from './settingsStore';

const SettingsContext = createContext<SettingsHandle | null>(null);

interface SettingsProviderProps {
	client: SettingsClient;
	children: ReactNode;
}

/**
 * Follows `client`'s settings for as long as it is mounted. Children render at once with the
 * defaults and re-render when Rust's answer arrives; a window that cannot reach the plugin simply
 * keeps the defaults.
 */
export function SettingsProvider({ client, children }: SettingsProviderProps) {
	const [handle, setHandle] = useState<SettingsHandle | null>(null);
	useEffect(() => {
		const created = createSettingsStore(client);
		setHandle(created);
		return () => created.dispose();
	}, [client]);
	return <SettingsContext.Provider value={handle}>{children}</SettingsContext.Provider>;
}

/** The window's settings handle, or `null` outside a provider and before the store exists. */
export function useSettingsHandle(): SettingsHandle | null {
	return useContext(SettingsContext);
}

const fallback = createStore<SettingsState>()(() => ({
	settings: DEFAULT_SETTINGS,
	revision: 0,
	ready: false,
}));

/**
 * Reads part of the settings, re-rendering when that part changes. Outside a provider it reads the
 * defaults, so a component works in a test or a window without the plugin.
 */
export function useSettings<T>(select: (settings: Settings) => T): T {
	const handle = useContext(SettingsContext);
	const selector = useMemo(() => (state: SettingsState) => select(state.settings), [select]);
	return useStore(handle?.store ?? fallback, selector);
}

/**
 * Whether the settings in `useSettings` are Rust's answer rather than the defaults it starts from.
 * Outside a provider the defaults are all there is, so that counts as ready.
 */
export function useSettingsReady(): boolean {
	const handle = useContext(SettingsContext);
	return useStore(handle?.store ?? fallback, (state) => handle === null || state.ready);
}
