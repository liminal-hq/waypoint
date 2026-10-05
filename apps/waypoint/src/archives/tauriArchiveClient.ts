// The archive client over Tauri: the app's `unlock_archive` command and its `archive-notice` event
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { ArchiveClient, SlowListing } from './archiveClient';

const NOTICE_EVENT = 'archive-notice';

export function createTauriArchiveClient(): ArchiveClient {
	return {
		unlock: (location: Location, passphrase: string) =>
			invoke<void>('unlock_archive', { location, passphrase }),
		onSlowListing(listener) {
			let stopped = false;
			let stop: (() => void) | null = null;
			void listen<SlowListing>(NOTICE_EVENT, (event) => listener(event.payload)).then(
				(unlisten) => {
					if (stopped) unlisten();
					else stop = unlisten;
				},
			);
			return () => {
				stopped = true;
				stop?.();
			};
		},
	};
}
