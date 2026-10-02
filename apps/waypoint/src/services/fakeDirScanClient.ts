// An in-memory DirScanClient for building and testing Overview without the Rust side
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { DirScanEvent } from '@liminal-hq/waypoint-protocol/generated/DirScanEvent';
import type { DirScanOptions } from '@liminal-hq/waypoint-protocol/generated/DirScanOptions';
import type { DirScanResult } from '@liminal-hq/waypoint-protocol/generated/DirScanResult';
import type { DirSizeRow } from '@liminal-hq/waypoint-protocol/generated/DirSizeRow';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { DirScanClient, DirScanJob } from './dirScanClient';

/** The remainder row's name, as the engine sends it. */
export const REMAINDER_NAME = 'Other files and folders, including hidden';

/** A `DirScanResult` for `root` from `[name, bytes]` pairs (the remainder is `null` as its name). */
export function fakeDirScanResult(
	root: Location,
	rows: ReadonlyArray<readonly [string | null, number]>,
	overrides: Partial<DirScanResult> = {},
): DirScanResult {
	const total = rows.reduce((sum, [, bytes]) => sum + bytes, 0);
	const built: DirSizeRow[] = rows.map(([name, bytes]) => ({
		name: name ?? REMAINDER_NAME,
		kind: name === null ? 'other' : 'folder',
		location:
			name === null ? null : { display: `${root.display}/${name}`, uri: `${root.uri}/${name}` },
		bytes,
		files: 1,
		share: total === 0 ? 0 : bytes / total,
	}));
	return {
		root,
		rows: built,
		totalBytes: total,
		totalFiles: built.length,
		allocated: false,
		foldersScanned: built.filter((r) => r.kind === 'folder').length,
		foldersTotal: built.filter((r) => r.kind === 'folder').length,
		symlinksSkipped: 0,
		mountsSkipped: 0,
		placeholders: 0,
		unreadable: 0,
		measuredAtMs: 0,
		...overrides,
	};
}

interface Run {
	root: Location;
	onEvent: (event: DirScanEvent) => void;
	latest: DirScanResult | null;
	ended: boolean;
}

/**
 * Follows the plugin's contract: a scan is a scripted one a test advances by hand (`partial`,
 * `finish`, `fail`) so progress is deterministic; `cancel` ends it with `cancelled` and the latest
 * partial result; `finish` caches its result like the engine does. `failNextScan` makes the next
 * `scan` reject.
 */
export class FakeDirScanClient implements DirScanClient {
	private readonly cache = new Map<string, DirScanResult>();
	private readonly runs = new Map<number, Run>();
	private nextJob = 1;
	private failure: VfsError | null = null;
	/** Every call made, in order, for tests to check what the UI asked for. */
	readonly calls: string[] = [];
	/** The options of the latest `scan`. */
	lastOptions: DirScanOptions | undefined;

	/** Seeds the cache for a root. */
	setCached(result: DirScanResult): void {
		this.cache.set(result.root.uri, result);
	}

	failNextScan(error: VfsError): void {
		this.failure = error;
	}

	async scan(
		location: Location,
		onEvent: (event: DirScanEvent) => void,
		options?: DirScanOptions,
	): Promise<DirScanJob> {
		this.calls.push(`scan ${location.uri}`);
		this.lastOptions = options;
		if (this.failure) {
			const error = this.failure;
			this.failure = null;
			throw error;
		}
		const job = this.nextJob++;
		this.runs.set(job, { root: location, onEvent, latest: null, ended: false });
		return { job, cancel: async () => this.end(job, 'cancelled') };
	}

	async cached(location: Location): Promise<DirScanResult | null> {
		this.calls.push(`cached ${location.uri}`);
		return this.cache.get(location.uri) ?? null;
	}

	/** Sends a progress event for a scan that is still going. */
	progress(job: number, current: string, scannedBytes: number): void {
		const run = this.live(job);
		if (run) run.onEvent({ kind: 'progress', current, scannedBytes });
	}

	/** Sends a partial result for a scan that is still going. */
	partial(job: number, result: DirScanResult): void {
		const run = this.live(job);
		if (!run) return;
		run.latest = result;
		run.onEvent({ kind: 'partial', result });
	}

	/** Ends a scan with `done` and `result`, which becomes the cached result for its root. */
	finish(job: number, result: DirScanResult): void {
		const run = this.live(job);
		if (!run) return;
		run.ended = true;
		this.cache.set(run.root.uri, result);
		run.onEvent({ kind: 'done', result });
		this.runs.delete(job);
	}

	/** Ends a scan with `failed`. */
	fail(job: number, error: VfsError): void {
		const run = this.live(job);
		if (!run) return;
		run.ended = true;
		run.onEvent({ kind: 'failed', error });
		this.runs.delete(job);
	}

	private live(job: number): Run | undefined {
		const run = this.runs.get(job);
		return run && !run.ended ? run : undefined;
	}

	private end(job: number, kind: 'cancelled'): void {
		const run = this.live(job);
		if (!run) return;
		run.ended = true;
		run.onEvent({
			kind,
			result: run.latest ?? fakeDirScanResult(run.root, [], { foldersTotal: 0 }),
		});
		this.runs.delete(job);
	}
}
