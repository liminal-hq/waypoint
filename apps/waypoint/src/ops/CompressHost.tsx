// Shows the compress dialog when something asks for a name and a format: mount one in each window that can compress
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect } from 'react';
import { useStore } from 'zustand';
import { CompressDialog } from './CompressDialog';
import { attachCompressHost, compressStore, type CompressStore } from './compressStore';

export function CompressHost({ store = compressStore }: { store?: CompressStore }) {
	const request = useStore(store, (state) => state.request);
	useEffect(() => attachCompressHost(store), [store]);
	if (!request) return null;
	return (
		<CompressDialog
			options={request.options}
			onConfirm={(choice) => request.resolve(choice)}
			onCancel={() => request.resolve(null)}
		/>
	);
}
