// Shows the application chooser when something asks for it: mount one in each window that has Open With
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useStore } from 'zustand';
import { showNotice } from '../app/notices';
import { useOpenWithService } from './OpenWithContext';
import { OpenWithDialog } from './OpenWithDialog';
import { openWithChooserStore, type OpenWithChooserStore } from './openWithChooserStore';
import { startOpenWith } from './startOpenWith';

export interface OpenWithHostProps {
	/** The store the dialog follows; the window's own by default. */
	store?: OpenWithChooserStore;
}

export function OpenWithHost({ store = openWithChooserStore }: OpenWithHostProps) {
	const request = useStore(store, (state) => state.request);
	const service = useOpenWithService();
	if (!request || !service) return null;
	const { client } = service;
	return (
		<OpenWithDialog
			request={request}
			iconUrl={(appId) => client.iconUrl(appId)}
			onCancel={() => store.getState().close()}
			onChoose={(app) => {
				store.getState().close();
				void startOpenWith(client, request.uris, { kind: 'app', app }, showNotice);
			}}
		/>
	);
}
