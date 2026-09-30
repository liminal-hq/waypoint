// The VfsClient the file views talk to, supplied by whoever hosts them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, type ReactNode } from 'react';
import type { VfsClient } from '../services/vfsClient';

const VfsClientContext = createContext<VfsClient | null>(null);

interface VfsClientProviderProps {
	client: VfsClient;
	children: ReactNode;
}

/** Makes `client` the file system every view below reads from (the fake now, the plugin later). */
export function VfsClientProvider({ client, children }: VfsClientProviderProps) {
	return <VfsClientContext.Provider value={client}>{children}</VfsClientContext.Provider>;
}

export function useVfsClient(): VfsClient {
	const client = useContext(VfsClientContext);
	if (!client) throw new Error('useVfsClient must be used inside a VfsClientProvider');
	return client;
}
