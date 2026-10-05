// Gives the window's views the archive client
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, type ReactNode } from 'react';
import type { ArchiveClient } from './archiveClient';

const ArchiveClientContext = createContext<ArchiveClient | null>(null);

export function ArchiveClientProvider({
	client,
	children,
}: {
	client: ArchiveClient | null;
	children: ReactNode;
}) {
	return <ArchiveClientContext.Provider value={client}>{children}</ArchiveClientContext.Provider>;
}

/** The archive client, or `null` where the window has none (a build without archives, most tests). */
export function useArchiveClient(): ArchiveClient | null {
	return useContext(ArchiveClientContext);
}
