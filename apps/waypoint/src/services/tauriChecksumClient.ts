// The real ChecksumClient: the app's checksum commands, streamed over a channel
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Channel, invoke } from '@tauri-apps/api/core';
import type { ChecksumEvent } from '@liminal-hq/waypoint-protocol/generated/ChecksumEvent';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { ChecksumClient } from './checksumClient';
import { isVfsError } from './vfsClient';

function asVfsError(error: unknown): VfsError {
	if (isVfsError(error)) return error;
	const message = error instanceof Error ? error.message : String(error);
	return { kind: 'io', message, location: null };
}

/** A `ChecksumClient` over the app's `file_checksum` and `cancel_checksum` commands. */
export function createTauriChecksumClient(): ChecksumClient {
	return {
		async start(handle, id, algorithm, onEvent) {
			const channel = new Channel<ChecksumEvent>();
			channel.onmessage = onEvent;
			try {
				const job = await invoke<number>('file_checksum', {
					handle,
					id,
					algorithm,
					onEvent: channel,
				});
				return { job, cancel: () => invoke<void>('cancel_checksum', { job }) };
			} catch (error) {
				throw asVfsError(error);
			}
		},
	};
}
