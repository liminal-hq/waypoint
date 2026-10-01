// An in-memory TearoffClient that records what the drag asks of the plugin and answers from a script
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	NO_TEAROFF,
	type BeginResult,
	type DragHover,
	type DragLeave,
	type DropReport,
	type GhostPayload,
	type Hit,
	type MergeHover,
	type PayloadDropped,
	type Point,
	type Region,
	type Size,
	type TearoffClient,
	type TearoffFeatures,
	type ToplevelBeginReport,
	type ToplevelDragEnded,
	type ToplevelDragStarted,
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
	/** What `beginToplevelDrag` answers; a `started` one also fires `onToplevelStarted`, as the plugin does. */
	toplevelBegin: ToplevelBeginReport = { state: 'started', reason: null };
	toplevelBegins: { payload: unknown; windowLabel: string; grabOffset: Point }[] = [];
	/** What `takeToplevelResult` returns, once. */
	pendingResult: ToplevelDragEnded | null = null;
	/** The values `holdNextWindow` was given, in order. */
	holds: boolean[] = [];
	/** The windows `showWindow` was asked to show. */
	shown: string[] = [];
	/** When set, `beginToplevelDrag` rejects with it, as a failed call to the plugin does. */
	toplevelBeginError: Error | null = null;
	/** What `sendMergeHover` and `sendMergeLeave` were asked to tell other windows, in order. */
	hoverSent: { window: string; hover: MergeHover }[] = [];
	leavesSent: string[] = [];
	/** Called for each send, so two fake windows can be linked into one. */
	onSend: ((message: { window: string; hover: MergeHover | null }) => void) | null = null;
	private startedListeners = new Set<(started: ToplevelDragStarted) => void>();
	private endedListeners = new Set<(ended: ToplevelDragEnded) => void>();
	private droppedListeners = new Set<(dropped: PayloadDropped) => void>();
	private mergeHoverListeners = new Set<(hover: MergeHover) => void>();
	private mergeLeaveListeners = new Set<() => void>();
	private dragHoverListeners = new Set<(hover: DragHover) => void>();
	private dragLeaveListeners = new Set<(leave: DragLeave) => void>();
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

	async sendMergeHover(window: string, hover: MergeHover): Promise<void> {
		this.calls.push(`mergeHover:${window}`);
		this.hoverSent.push({ window, hover });
		this.onSend?.({ window, hover });
	}

	async sendMergeLeave(window: string): Promise<void> {
		this.calls.push(`mergeLeave:${window}`);
		this.leavesSent.push(window);
		this.onSend?.({ window, hover: null });
	}

	onMergeHover(listener: (hover: MergeHover) => void): () => void {
		this.mergeHoverListeners.add(listener);
		return () => this.mergeHoverListeners.delete(listener);
	}

	onMergeLeave(listener: () => void): () => void {
		this.mergeLeaveListeners.add(listener);
		return () => this.mergeLeaveListeners.delete(listener);
	}

	onDragHover(listener: (hover: DragHover) => void): () => void {
		this.dragHoverListeners.add(listener);
		return () => this.dragHoverListeners.delete(listener);
	}

	onDragLeave(listener: (leave: DragLeave) => void): () => void {
		this.dragLeaveListeners.add(listener);
		return () => this.dragLeaveListeners.delete(listener);
	}

	fireMergeHover(hover: MergeHover): void {
		for (const listener of [...this.mergeHoverListeners]) listener(hover);
	}

	fireMergeLeave(): void {
		for (const listener of [...this.mergeLeaveListeners]) listener();
	}

	fireDragHover(hover: DragHover): void {
		for (const listener of [...this.dragHoverListeners]) listener(hover);
	}

	fireDragLeave(leave: DragLeave): void {
		for (const listener of [...this.dragLeaveListeners]) listener(leave);
	}

	async holdNextWindow(on: boolean): Promise<void> {
		this.calls.push(`hold:${on}`);
		this.holds.push(on);
	}

	async showWindow(label: string): Promise<void> {
		this.calls.push(`show:${label}`);
		this.shown.push(label);
	}

	async beginToplevelDrag(
		payload: unknown,
		windowLabel: string,
		grabOffset: Point,
	): Promise<ToplevelBeginReport> {
		this.calls.push('beginToplevel');
		this.toplevelBegins.push({ payload, windowLabel, grabOffset });
		await this.gate;
		if (this.toplevelBeginError) throw this.toplevelBeginError;
		if (this.toplevelBegin.state === 'started') {
			for (const listener of [...this.startedListeners]) listener({ window: windowLabel, payload });
		}
		return this.toplevelBegin;
	}

	async endToplevelDrag(): Promise<void> {
		this.calls.push('endToplevel');
	}

	async takeToplevelResult(): Promise<ToplevelDragEnded | null> {
		this.calls.push('takeResult');
		const result = this.pendingResult;
		this.pendingResult = null;
		return result;
	}

	onToplevelStarted(listener: (started: ToplevelDragStarted) => void): () => void {
		this.startedListeners.add(listener);
		return () => this.startedListeners.delete(listener);
	}

	onToplevelEnded(listener: (ended: ToplevelDragEnded) => void): () => void {
		this.endedListeners.add(listener);
		return () => this.endedListeners.delete(listener);
	}

	onPayloadDropped(listener: (dropped: PayloadDropped) => void): () => void {
		this.droppedListeners.add(listener);
		return () => this.droppedListeners.delete(listener);
	}

	fireEnded(ended: ToplevelDragEnded): void {
		for (const listener of [...this.endedListeners]) listener(ended);
	}

	fireDropped(dropped: PayloadDropped): void {
		for (const listener of [...this.droppedListeners]) listener(dropped);
	}

	fireTimeout(): void {
		for (const listener of [...this.timeoutListeners]) listener();
	}

	fireStale(stale: boolean): void {
		for (const listener of [...this.staleListeners]) listener(stale);
	}
}
