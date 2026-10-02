// The one measurement of Home a window runs: its rows as they arrive, its cached result, and starting and cancelling it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { DirScanEvent } from '@liminal-hq/waypoint-protocol/generated/DirScanEvent';
import type { DirScanResult } from '@liminal-hq/waypoint-protocol/generated/DirScanResult';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { DirScanClient, DirScanJob } from '../services/dirScanClient';

/** How a measurement of Home last ended. */
export type ScanOutcome = 'none' | 'done' | 'cancelled' | 'failed';

export interface HomeScanState {
	/** The folder being measured, or last measured. */
	root: Location | null;
	/** Whether a scan is running (or starting). At most one is, ever. */
	running: boolean;
	/** The rows to show: the scan's partial result while it runs, else its last result (cached or finished, or what a cancelled one reached). */
	shown: DirScanResult | null;
	/** The last complete result, cached or finished: what the Home stat and the "Your files" segment are read from. */
	final: DirScanResult | null;
	/** The top-level folder the scan is in now. */
	current: string | null;
	outcome: ScanOutcome;
	/** Why the scan failed, when `outcome` is `failed`. */
	error: string | null;
}

const IDLE: HomeScanState = {
	root: null,
	running: false,
	shown: null,
	final: null,
	current: null,
	outcome: 'none',
	error: null,
};

/**
 * Owns the window's measurement of Home so Overview, the status bar and the Home stat read one
 * state and a second scan can never start beside the first. `cancel` ends the run in the state at
 * once (a late event from the engine is ignored) and tells the engine, which stops within its
 * budget; `start` while a scan is running does nothing.
 */
export class HomeScanStore {
	private state: HomeScanState = IDLE;
	private readonly listeners = new Set<() => void>();
	/** Numbers each run, so events of a run that was cancelled or replaced are ignored. */
	private run = 0;
	private job: DirScanJob | null = null;

	constructor(private readonly client: DirScanClient) {}

	subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	getSnapshot = (): HomeScanState => this.state;

	/** Reads the cached result for `root` so "as of <time>" shows at once. A running scan's rows are not replaced. */
	async load(root: Location): Promise<DirScanResult | null> {
		if (this.state.root?.uri !== root.uri && !this.state.running) {
			this.set({ ...IDLE, root });
		}
		let cached: DirScanResult | null = null;
		try {
			cached = await this.client.cached(root);
		} catch (error) {
			console.warn('could not read the cached measurement of Home', error);
		}
		if (cached && this.state.root?.uri === root.uri && !this.state.final) {
			this.set({
				...this.state,
				final: cached,
				shown: this.state.running ? this.state.shown : cached,
			});
		}
		return cached;
	}

	/** Starts measuring `root`. Returns `false` when a scan is already running. */
	start(root: Location): boolean {
		if (this.state.running) return false;
		const run = ++this.run;
		this.job = null;
		const sameRoot = this.state.root?.uri === root.uri;
		this.set({
			...(sameRoot ? this.state : { ...IDLE, root }),
			root,
			running: true,
			current: null,
			outcome: 'none',
			error: null,
		});
		this.client
			.scan(root, (event) => this.onEvent(run, event))
			.then(
				(job) => {
					if (run !== this.run) {
						// Cancelled before it had started: stop the run that has now begun.
						void job.cancel().catch(() => {});
						return;
					}
					this.job = job;
				},
				(error: unknown) => {
					if (run !== this.run) return;
					this.fail(error);
				},
			);
		return true;
	}

	/** Stops the running scan; the rows it had reached stay shown. */
	cancel(): void {
		if (!this.state.running) return;
		this.run += 1;
		const job = this.job;
		this.job = null;
		// Before the engine has answered there is no job yet: `start` stops it when it arrives.
		if (job) void job.cancel().catch(() => {});
		this.set({ ...this.state, running: false, current: null, outcome: 'cancelled' });
	}

	private onEvent(run: number, event: DirScanEvent): void {
		if (run !== this.run) return;
		switch (event.kind) {
			case 'progress':
				this.set({ ...this.state, current: event.current });
				break;
			case 'partial':
				this.set({ ...this.state, shown: event.result });
				break;
			case 'done':
				this.job = null;
				this.set({
					...this.state,
					running: false,
					shown: event.result,
					final: event.result,
					current: null,
					outcome: 'done',
				});
				break;
			case 'cancelled':
				this.job = null;
				this.set({
					...this.state,
					running: false,
					shown: event.result,
					current: null,
					outcome: 'cancelled',
				});
				break;
			case 'failed':
				this.job = null;
				this.fail(event.error);
				break;
		}
	}

	private fail(error: unknown): void {
		const message =
			typeof error === 'object' && error !== null && 'message' in error
				? String((error as { message: unknown }).message)
				: String(error);
		this.set({
			...this.state,
			running: false,
			current: null,
			outcome: 'failed',
			error: message,
		});
	}

	private set(next: HomeScanState): void {
		this.state = next;
		this.listeners.forEach((listener) => listener());
	}
}
