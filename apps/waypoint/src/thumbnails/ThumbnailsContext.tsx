// The ThumbnailsClient a window uses, whether the plugin works here, and the loaders the views make from them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useSettings } from '../settings/SettingsContext';
import { ThumbnailLoader, type LoaderTransport } from './thumbnailLoader';
import type {
	EntryRequest,
	LocationRequest,
	PluginStatus,
	ThumbnailsClient,
	ThumbSize,
} from './thumbnailsClient';

interface ThumbnailsValue {
	client: ThumbnailsClient;
	/** What the plugin reported at start; `null` until it has answered, and when it could not. */
	status: PluginStatus | null;
}

const ThumbnailsContext = createContext<ThumbnailsValue | null>(null);

interface ThumbnailsProviderProps {
	client: ThumbnailsClient | undefined;
	children: ReactNode;
}

/**
 * Makes `client` the thumbnails service every view below uses, and reads the plugin's status once
 * so they ask for nothing where it cannot work (the Services panel says why). Without a client
 * (the demo, a test) every view keeps its icons.
 */
export function ThumbnailsProvider({ client, children }: ThumbnailsProviderProps) {
	const [status, setStatus] = useState<PluginStatus | null>(null);
	useEffect(() => {
		if (!client) return;
		let live = true;
		client.getStatus().then(
			(next) => {
				if (live) setStatus(next);
			},
			(error: unknown) => console.warn('could not read the thumbnails status', error),
		);
		return () => {
			live = false;
		};
	}, [client]);
	const value = useMemo(() => (client ? { client, status } : null), [client, status]);
	return <ThumbnailsContext.Provider value={value}>{children}</ThumbnailsContext.Provider>;
}

/** The window's client and status, or `null` where the host has none. */
export function useThumbnailService(): ThumbnailsValue | null {
	return useContext(ThumbnailsContext);
}

/**
 * The client to ask for thumbnails, or `null` when none should be asked for: no client, the plugin
 * reports itself unavailable, or "Show thumbnails" is off. Views keep their icons then.
 */
export function useActiveThumbnailsClient(): ThumbnailsClient | null {
	const service = useContext(ThumbnailsContext);
	const enabled = useSettings(selectEnabled);
	return service && service.status?.available === true && enabled ? service.client : null;
}

const selectEnabled = (settings: { previews: { thumbnails: boolean } }) =>
	settings.previews.thumbnails;

/** The device's pixel ratio, for choosing a size that is not scaled up. */
export function devicePixelRatio(): number {
	return typeof window === 'undefined' ? 1 : (window.devicePixelRatio ?? 1);
}

function useLoader<Item extends { key: string }>(
	transport: LoaderTransport<Item> | null,
	size: ThumbSize | null,
): ThumbnailLoader<Item> | null {
	const [loader, setLoader] = useState<ThumbnailLoader<Item> | null>(null);
	useEffect(() => {
		if (!transport || size === null) {
			setLoader(null);
			return;
		}
		const created = new ThumbnailLoader(transport, size);
		setLoader(created);
		return () => {
			created.dispose();
			setLoader((current) => (current === created ? null : current));
		};
	}, [transport, size]);
	return loader;
}

/** A loader for the entries of the listing `handle`, at `size`; `null` while nothing should be asked for. */
export function useEntryThumbnailLoader(
	handle: ListingHandle,
	size: ThumbSize | null,
): ThumbnailLoader<EntryRequest> | null {
	const client = useActiveThumbnailsClient();
	const transport = useMemo<LoaderTransport<EntryRequest> | null>(
		() =>
			client && {
				request: (items, itemSize, onEvent) =>
					client.requestEntries(handle, items, itemSize, onEvent),
				prioritise: (ticket, keys) => client.prioritise(ticket, keys),
				cancel: (ticket) => client.cancel(ticket),
			},
		[client, handle],
	);
	return useLoader(transport, size);
}

/** A loader for locations (the Shelf's items), at `size`. */
export function useLocationThumbnailLoader(
	size: ThumbSize | null,
): ThumbnailLoader<LocationRequest> | null {
	const client = useActiveThumbnailsClient();
	const transport = useMemo<LoaderTransport<LocationRequest> | null>(
		() =>
			client && {
				request: (items, itemSize, onEvent) => client.requestLocations(items, itemSize, onEvent),
				prioritise: (ticket, keys) => client.prioritise(ticket, keys),
				cancel: (ticket) => client.cancel(ticket),
			},
		[client],
	);
	return useLoader(transport, size);
}
