// Shows Quick Look when a view asks for it: mount one in each window that can preview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import { useStore } from 'zustand';
import { useDetailsClient } from '../inspector/DetailsClientContext';
import { QuickLook } from './QuickLook';
import { quickLookStore, type QuickLookStore } from './quickLookStore';

export interface QuickLookHostProps {
	/** The store the overlay follows; the window's own by default. */
	store?: QuickLookStore;
}

/** Without a details client (the demo, a test) the host is not attached, so Space does nothing. */
export function QuickLookHost({ store = quickLookStore }: QuickLookHostProps) {
	const client = useDetailsClient();
	useEffect(() => (client ? store.getState().attach() : undefined), [client, store]);
	const request = useStore(store, (state) => state.request);
	if (!client || !request) return null;
	return (
		<QuickLook
			session={request.session}
			move={request.move}
			client={client}
			onOpen={request.onOpen}
			onClose={() => store.getState().close()}
		/>
	);
}
