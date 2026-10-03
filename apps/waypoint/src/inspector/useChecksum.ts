// One checksum run: idle until asked, then progress, then its digest, a cancellation or a failure
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ChecksumEvent } from '@liminal-hq/waypoint-protocol/generated/ChecksumEvent';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { VerifyAlgorithm } from '@liminal-hq/waypoint-protocol/generated/VerifyAlgorithm';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { ChecksumClient, ChecksumRun } from '../services/checksumClient';
import { isVfsError } from '../services/vfsClient';

export type ChecksumState =
	| { status: 'idle' }
	| { status: 'running'; algorithm: VerifyAlgorithm; bytesRead: number; total: number }
	| { status: 'done'; algorithm: VerifyAlgorithm; digest: string; bytes: number }
	| { status: 'cancelled' }
	| { status: 'failed'; error: VfsError };

export interface ChecksumControl {
	state: ChecksumState;
	/** Starts a run; nothing starts a checksum but this (it is never automatic). */
	start(algorithm: VerifyAlgorithm): void;
	/** Stops the run that is going, if one is. */
	cancel(): void;
	/** Forgets a result (the file changed, or another algorithm was picked). */
	reset(): void;
}

/**
 * Drives the checksum of entry `id` of `handle`. A run belongs to the subject it was started for:
 * when `subjectKey` changes the run is cancelled and the result forgotten, so a digest is never
 * shown under another file's name, and leaving cancels the run too. Events from a run that is no
 * longer the current one are ignored.
 */
export function useChecksum(
	client: ChecksumClient | null,
	handle: ListingHandle | null,
	id: EntryId | null,
	subjectKey: string,
): ChecksumControl {
	const [state, setState] = useState<ChecksumState>({ status: 'idle' });
	const current = useRef<{ token: number; run: ChecksumRun | null } | null>(null);
	const counter = useRef(0);

	const stop = useCallback(() => {
		const active = current.current;
		current.current = null;
		if (active?.run) void active.run.cancel().catch(() => {});
	}, []);

	useEffect(() => {
		setState({ status: 'idle' });
		return stop;
	}, [subjectKey, stop]);

	const start = useCallback(
		(algorithm: VerifyAlgorithm) => {
			if (!client || handle === null || id === null) return;
			stop();
			const token = ++counter.current;
			const mine = { token, run: null as ChecksumRun | null };
			current.current = mine;
			setState({ status: 'running', algorithm, bytesRead: 0, total: 0 });
			const onEvent = (event: ChecksumEvent) => {
				if (current.current !== mine) return;
				switch (event.kind) {
					case 'progress':
						setState({
							status: 'running',
							algorithm,
							bytesRead: event.bytesRead,
							total: event.total,
						});
						return;
					case 'done':
						current.current = null;
						setState({
							status: 'done',
							algorithm: event.algorithm,
							digest: event.digest,
							bytes: event.bytes,
						});
						return;
					case 'cancelled':
						current.current = null;
						setState({ status: 'cancelled' });
						return;
					case 'failed':
						current.current = null;
						setState({ status: 'failed', error: event.error });
						return;
				}
			};
			client.start(handle, id, algorithm, onEvent).then(
				(run) => {
					if (current.current === mine) mine.run = run;
					else void run.cancel().catch(() => {});
				},
				(error: unknown) => {
					if (current.current !== mine) return;
					current.current = null;
					setState({
						status: 'failed',
						error: isVfsError(error)
							? error
							: { kind: 'io', message: String(error), location: null },
					});
				},
			);
		},
		[client, handle, id, stop],
	);

	const cancel = useCallback(() => {
		const active = current.current;
		if (!active?.run) {
			// Not started yet: nothing to ask Rust to stop, so drop the run and say so.
			current.current = null;
			setState((now) => (now.status === 'running' ? { status: 'cancelled' } : now));
			return;
		}
		// The run ends with its own `cancelled` event, which sets the state.
		void active.run.cancel().catch(() => {});
	}, []);

	const reset = useCallback(() => {
		stop();
		setState({ status: 'idle' });
	}, [stop]);

	return { state, start, cancel, reset };
}
