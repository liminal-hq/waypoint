// Supplies the window's Git store, and the hooks that read it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GitBadge } from '@liminal-hq/waypoint-protocol/generated/GitBadge';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import {
	createContext,
	useContext,
	useEffect,
	useMemo,
	useState,
	useSyncExternalStore,
	type ReactNode,
} from 'react';
import { useSettings } from '../settings/SettingsContext';
import type { GitClient } from '../services/gitClient';
import { GitStore, type Repository } from './gitStore';

interface GitHandle {
	client: GitClient;
	store: GitStore;
}

const GitContext = createContext<GitHandle | null>(null);

interface GitProviderProps {
	client: GitClient | undefined;
	children: ReactNode;
}

/**
 * Follows `client`'s repositories for as long as it is mounted. Without a client (the in-memory
 * demo, a window that cannot reach the plugin) there is no Git anywhere in the window.
 */
export function GitProvider({ client, children }: GitProviderProps) {
	const [handle, setHandle] = useState<GitHandle | null>(null);
	useEffect(() => {
		if (!client) {
			setHandle(null);
			return;
		}
		const store = new GitStore(client);
		setHandle({ client, store });
		return () => store.dispose();
	}, [client]);
	return <GitContext.Provider value={handle}>{children}</GitContext.Provider>;
}

/** The window's Git service, or `null` without one. */
export function useGitClient(): GitClient | null {
	return useContext(GitContext)?.client ?? null;
}

/** Whether the Settings switch has the Git status on. */
export function useGitEnabled(): boolean {
	return useSettings(selectGitDecorations);
}

const selectGitDecorations = (settings: { general: { gitDecorations: boolean } }) =>
	settings.general.gitDecorations;

const NO_STORE_SUBSCRIBE = () => () => {};

/**
 * The repository `location` is in, once the plugin has found it: `null` outside a working tree, for
 * a folder that is not local, while the plugin is still looking, with no Git service, or with the
 * Settings switch off (then nothing is watched at all).
 */
export function useRepository(location: Location | undefined): Repository | null {
	const handle = useContext(GitContext);
	const enabled = useGitEnabled();
	const store = handle?.store;
	const uri = location?.uri;
	useEffect(() => {
		if (!store || !location || !enabled) return;
		return store.acquire(location);
		// The uri names the folder; the location object may be a new one with the same uri.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [store, uri, enabled]);
	useSyncExternalStore(store?.subscribe ?? NO_STORE_SUBSCRIBE, store?.getVersion ?? zero);
	if (!store || !location || !enabled) return null;
	return store.repository(location);
}

const zero = () => 0;

/** A number that changes whenever any repository the window watches does: what a reader of the badges refreshes on. */
export function useGitVersion(): number {
	const store = useContext(GitContext)?.store;
	return useSyncExternalStore(store?.subscribe ?? NO_STORE_SUBSCRIBE, store?.getVersion ?? zero);
}

/** How many paths changed inside each of `locations` (the sidebar's folders), refreshed when `refreshKey` changes. */
export function useGitBadges(
	locations: readonly Location[],
	refreshKey: unknown,
): ReadonlyMap<string, GitBadge> {
	const handle = useContext(GitContext);
	const enabled = useGitEnabled();
	const [badges, setBadges] = useState<ReadonlyMap<string, GitBadge>>(new Map());
	// A window that regains focus has probably seen changes made elsewhere: read again.
	const [focuses, setFocuses] = useState(0);
	useEffect(() => {
		const onFocus = () => setFocuses((count) => count + 1);
		window.addEventListener('focus', onFocus);
		return () => window.removeEventListener('focus', onFocus);
	}, []);
	const key = locations.map((location) => location.uri).join('\n');
	const client = handle?.client;
	const asked = useMemo(
		() =>
			key === '' ? [] : locations.filter((location) => key.split('\n').includes(location.uri)),
		// `locations` itself is a new array each render; its uris are the key.
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[key],
	);
	useEffect(() => {
		if (!client || !enabled || asked.length === 0) {
			setBadges((current) => (current.size === 0 ? current : new Map()));
			return;
		}
		let live = true;
		client.badges([...asked]).then(
			(found) => {
				if (live) setBadges(new Map(found.map((badge) => [badge.uri, badge])));
			},
			(error: unknown) => console.warn('could not read the Git badges', error),
		);
		return () => {
			live = false;
		};
	}, [client, enabled, asked, refreshKey, focuses]);
	return badges;
}
