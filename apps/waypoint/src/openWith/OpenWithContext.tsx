// The OpenWithClient a window uses and what it reported at start, supplied by whoever hosts the window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { hasFeature, type PluginStatus } from '@liminal-hq/plugin-mime-apps';
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import type { OpenWithClient } from './openWithClient';

interface OpenWithValue {
	client: OpenWithClient;
	/** What the plugin can do here; `null` until it has answered, and when it could not. */
	status: PluginStatus | null;
}

const OpenWithContext = createContext<OpenWithValue | null>(null);

interface OpenWithProviderProps {
	client: OpenWithClient | undefined;
	children: ReactNode;
}

/**
 * Makes `client` the Open With service every menu and command below uses, and reads the plugin's
 * status once so they can hide what does not work here. Without a client (the demo, a test) Open
 * With is not offered.
 */
export function OpenWithProvider({ client, children }: OpenWithProviderProps) {
	const [status, setStatus] = useState<PluginStatus | null>(null);
	useEffect(() => {
		if (!client) return;
		let live = true;
		client.getStatus().then(
			(next) => {
				if (live) setStatus(next);
			},
			(error: unknown) => console.warn('could not read the Open With status', error),
		);
		return () => {
			live = false;
		};
	}, [client]);
	const value = useMemo(() => (client ? { client, status } : null), [client, status]);
	return <OpenWithContext.Provider value={value}>{children}</OpenWithContext.Provider>;
}

/** The window's client and status, or `null` where the host has none. */
export function useOpenWithService(): OpenWithValue | null {
	return useContext(OpenWithContext);
}

/** What Open With can do here, from the plugin's features: list applications, open the default, and use the system's chooser. */
export interface OpenWithAbilities {
	/** Applications can be listed and one chosen to open with. */
	list: boolean;
	/** The default application can be opened. */
	openDefault: boolean;
	/** The system has its own chooser. */
	chooser: boolean;
}

/** The abilities in `status`; all false before it has been read. */
export function openWithAbilities(status: PluginStatus | null): OpenWithAbilities {
	if (!status) return { list: false, openDefault: false, chooser: false };
	return {
		list: hasFeature(status, 'handlers') && hasFeature(status, 'openWith'),
		openDefault: hasFeature(status, 'openDefault'),
		chooser: hasFeature(status, 'chooser'),
	};
}

/** True when Open With can do anything at all here. */
export function openWithOffered(status: PluginStatus | null): boolean {
	const { list, openDefault, chooser } = openWithAbilities(status);
	return list || openDefault || chooser;
}
