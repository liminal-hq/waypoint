// A scripted ChecksumClient: a test advances, finishes or fails each run by hand
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ChecksumEvent } from '@liminal-hq/waypoint-protocol/generated/ChecksumEvent';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { VerifyAlgorithm } from '@liminal-hq/waypoint-protocol/generated/VerifyAlgorithm';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { ChecksumClient, ChecksumRun } from './checksumClient';

interface Run {
	handle: ListingHandle;
	id: EntryId;
	algorithm: VerifyAlgorithm;
	onEvent: (event: ChecksumEvent) => void;
	ended: boolean;
}

/** Follows the contract: exactly one ending event per run, and `cancel` ends it `cancelled`. */
export class FakeChecksumClient implements ChecksumClient {
	readonly runs = new Map<number, Run>();
	/** Every start, in order, as `handle:id:algorithm`. */
	readonly calls: string[] = [];
	private nextJob = 1;
	private refusal: VfsError | null = null;

	/** Makes the next start reject with this error. */
	refuseNext(error: VfsError): void {
		this.refusal = error;
	}

	async start(
		handle: ListingHandle,
		id: EntryId,
		algorithm: VerifyAlgorithm,
		onEvent: (event: ChecksumEvent) => void,
	): Promise<ChecksumRun> {
		this.calls.push(`${handle}:${id}:${algorithm}`);
		if (this.refusal) {
			const error = this.refusal;
			this.refusal = null;
			throw error;
		}
		const job = this.nextJob++;
		this.runs.set(job, { handle, id, algorithm, onEvent, ended: false });
		return { job, cancel: async () => this.end(job, { kind: 'cancelled' }) };
	}

	advance(job: number, bytesRead: number, total: number): void {
		const run = this.runs.get(job);
		if (run && !run.ended) run.onEvent({ kind: 'progress', bytesRead, total });
	}

	finish(job: number, digest: string, bytes: number): void {
		const run = this.runs.get(job);
		if (run) this.end(job, { kind: 'done', algorithm: run.algorithm, digest, bytes });
	}

	fail(job: number, error: VfsError): void {
		this.end(job, { kind: 'failed', error });
	}

	private end(job: number, event: ChecksumEvent): void {
		const run = this.runs.get(job);
		if (!run || run.ended) return;
		run.ended = true;
		run.onEvent(event);
	}

	/** The jobs that have not ended. */
	get live(): number[] {
		return [...this.runs].filter(([, run]) => !run.ended).map(([job]) => job);
	}
}
