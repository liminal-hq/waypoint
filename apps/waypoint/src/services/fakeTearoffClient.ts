// An in-memory TearoffClient that records what the drag asks of the plugin and answers from a script
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	NO_TEAROFF,
	type BeginResult,
	type DropReport,
	type GhostPayload,
	type Hit,
	type Point,
	type Region,
	type Size,
	type TearoffClient,
	type TearoffFeatures,
} from './tearoffClient';

/** A cursor report at logical (`x`, `y`) on a window scaled by `scale`. */
export function reportAt(x: number, y: number, scale = 1, hit: Hit | null = null): DropReport {
	return {
		cursor: { x: x * scale, y: y * scale },
		scaleFactor: scale,
		cursorStale: false,
		hit,
	};
}

/**
 * The plugin as a test script: set `featureSet`, `beginResult`, `hit` and `report` before the
 * drag, then read `calls` for what happened and call `fireTimeout` or `fireStale` for its events.
 */
export class FakeTearoffClient implements TearoffClient {
	featureSet: TearoffFeatures;
	beginResult: BeginResult = 'following';
	/** What `hitTest` finds while the drag runs. */
	hit: Hit | null = null;
	/** What `end('drop')` reports; a cancel never has a hit. */
	report: DropReport = reportAt(0, 0);
	/** `end` and `begin` wait on this when set, to test a drag that moves on while a call is in flight. */
	gate: Promise<void> | null = null;
	regions: Region[] = [];
	calls: string[] = [];
	begins: { payload: GhostPayload; grabOffset: Point; size: Size }[] = [];
	updates: GhostPayload[] = [];
	statusCalls = 0;
	private timeoutListeners = new Set<() => void>();
	private staleListeners = new Set<(stale: boolean) => void>();

	constructor(features: Partial<TearoffFeatures> = {}) {
		this.featureSet = { ...NO_TEAROFF, ...features };
	}

	async features(): Promise<TearoffFeatures> {
		this.statusCalls++;
		return this.featureSet;
	}

	async begin(payload: GhostPayload, grabOffset: Point, size: Size): Promise<BeginResult> {
		this.calls.push('begin');
		this.begins.push({ payload, grabOffset, size });
		await this.gate;
		return this.beginResult;
	}

	async update(payload: GhostPayload): Promise<void> {
		this.calls.push('update');
		this.updates.push(payload);
	}

	async end(outcome: 'drop' | 'cancel'): Promise<DropReport> {
		this.calls.push(`end:${outcome}`);
		await this.gate;
		return outcome === 'cancel' ? { ...this.report, hit: null } : this.report;
	}

	async hitTest(): Promise<Hit | null> {
		this.calls.push('hitTest');
		return this.hit;
	}

	async setDropRegions(regions: Region[]): Promise<void> {
		this.calls.push('setDropRegions');
		this.regions = regions;
	}

	onTimeout(listener: () => void): () => void {
		this.timeoutListeners.add(listener);
		return () => this.timeoutListeners.delete(listener);
	}

	onCursorStale(listener: (stale: boolean) => void): () => void {
		this.staleListeners.add(listener);
		return () => this.staleListeners.delete(listener);
	}

	fireTimeout(): void {
		for (const listener of [...this.timeoutListeners]) listener();
	}

	fireStale(stale: boolean): void {
		for (const listener of [...this.staleListeners]) listener(stale);
	}
}
