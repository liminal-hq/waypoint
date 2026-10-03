// An in-memory NativeDndClient: a script for drags from other applications and a stand-in for the system's drag out
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	NO_NATIVE_DND,
	type DragAction,
	type DragEnded,
	type DragOutcome,
	type DropEvent,
	type EnterEvent,
	type ErrorKind,
	type LeaveEvent,
	type Modifiers,
	type NativeDndAvailability,
	type NativeDndClient,
	type OutboundRequest,
	type OutboundStarted,
	type OverEvent,
	type Position,
} from './nativeDndClient';
import type { Unsubscribe } from './vfsClient';

export interface FakeNativeDndOptions {
	/** What works; everything, as on X11, when omitted. Give `modifiers: false` for Wayland. */
	availability?: Partial<NativeDndAvailability>;
	/** The label of the window the events are for. */
	window?: string;
	/** `startDrag` rejects with this kind, as the plugin does without a pressed button. */
	refuseStart?: ErrorKind;
	/** `startDrag` resolves only when the drag has ended, with the result (Windows); the test ends it with `endDrag`. */
	modal?: boolean;
}

const NO_KEYS: Modifiers = { ctrl: false, shift: false, alt: false };

/** The plugin's events as a test scripts them, and what Waypoint asked it to do. */
export class FakeNativeDndClient implements NativeDndClient {
	private readonly availability: NativeDndAvailability;
	private readonly window: string;
	private readonly enters = new Set<(event: EnterEvent) => void>();
	private readonly overs = new Set<(event: OverEvent) => void>();
	private readonly drops = new Set<(event: DropEvent) => void>();
	private readonly leaves = new Set<(event: LeaveEvent) => void>();
	private readonly ends = new Set<(event: DragEnded) => void>();
	private nextId = 1;
	private settle: ((started: OutboundStarted) => void) | null = null;
	refuseStart: ErrorKind | undefined;
	modal: boolean;
	/** The outbound drags Waypoint started, in order. */
	readonly started: Array<OutboundRequest & { id: number }> = [];
	/** Every command, in order: `['startDrag', request]`. */
	readonly calls: unknown[][] = [];

	constructor(options: FakeNativeDndOptions = {}) {
		this.availability = {
			inbound: true,
			outbound: true,
			modifiers: true,
			clipboard: true,
			reasons: {},
			displayServer: 'x11',
			...options.availability,
		};
		this.window = options.window ?? 'main-1';
		this.refuseStart = options.refuseStart;
		this.modal = options.modal ?? false;
	}

	async status(): Promise<NativeDndAvailability> {
		return { ...this.availability };
	}

	startDrag(request: OutboundRequest): Promise<OutboundStarted> {
		this.calls.push(['startDrag', structuredClone(request)]);
		if (!this.availability.outbound) {
			return Promise.reject({ kind: 'unsupported', message: 'no outbound drag' });
		}
		if (this.refuseStart) {
			return Promise.reject({ kind: this.refuseStart, message: `refused: ${this.refuseStart}` });
		}
		const id = this.nextId++;
		this.started.push({ ...structuredClone(request), id });
		if (this.modal) {
			return new Promise((resolve) => {
				this.settle = resolve;
			});
		}
		return Promise.resolve({ id, ended: null });
	}

	onEnter(listener: (event: EnterEvent) => void): Unsubscribe {
		return this.add(this.enters, listener);
	}
	onOver(listener: (event: OverEvent) => void): Unsubscribe {
		return this.add(this.overs, listener);
	}
	onDrop(listener: (event: DropEvent) => void): Unsubscribe {
		return this.add(this.drops, listener);
	}
	onLeave(listener: (event: LeaveEvent) => void): Unsubscribe {
		return this.add(this.leaves, listener);
	}
	onDragEnded(listener: (event: DragEnded) => void): Unsubscribe {
		return this.add(this.ends, listener);
	}

	private add<T>(set: Set<(event: T) => void>, listener: (event: T) => void): Unsubscribe {
		set.add(listener);
		return () => {
			set.delete(listener);
		};
	}

	// --- The script ----------------------------------------------------------------------------

	/** Files first move over the window. `paths` are the display strings, which default to the URIs' own text. */
	enter(
		uris: string[],
		position: Position,
		options: {
			paths?: string[];
			modifiers?: Partial<Modifiers>;
			action?: DragAction | null;
			window?: string;
		} = {},
	): void {
		const event: EnterEvent = {
			window: options.window ?? this.window,
			uris,
			paths: options.paths ?? uris.map(pathOf),
			position,
			modifiers: this.keys(options.modifiers),
			action: options.action ?? null,
		};
		for (const listener of [...this.enters]) listener(event);
	}

	over(
		position: Position,
		options: {
			modifiers?: Partial<Modifiers>;
			action?: DragAction | null;
			window?: string;
		} = {},
	): void {
		const event: OverEvent = {
			window: options.window ?? this.window,
			position,
			modifiers: this.keys(options.modifiers),
			action: options.action ?? null,
		};
		for (const listener of [...this.overs]) listener(event);
	}

	drop(
		uris: string[],
		position: Position,
		options: {
			paths?: string[];
			modifiers?: Partial<Modifiers>;
			action?: DragAction | null;
			selfDrop?: boolean;
			window?: string;
		} = {},
	): void {
		const event: DropEvent = {
			window: options.window ?? this.window,
			uris,
			paths: options.paths ?? uris.map(pathOf),
			position,
			modifiers: this.keys(options.modifiers),
			action: options.action ?? null,
			selfDrop: options.selfDrop ?? false,
		};
		for (const listener of [...this.drops]) listener(event);
	}

	leave(options: { window?: string } = {}): void {
		for (const listener of [...this.leaves]) listener({ window: options.window ?? this.window });
	}

	/** The system's drag that Waypoint started has ended with `outcome`. */
	endDrag(outcome: DragOutcome, reason: string | null = null, id?: number): void {
		const last = this.started.at(-1);
		const event: DragEnded = {
			id: id ?? last?.id ?? 0,
			outcome,
			uris: last?.uris ?? [],
			reason,
		};
		// A modal drag (Windows) resolves the command with the end as well as sending the event.
		this.settle?.({ id: event.id, ended: event });
		this.settle = null;
		for (const listener of [...this.ends]) listener(event);
	}

	/** How many listeners are attached, for tests that check a window stops listening. */
	listenerCount(): number {
		return this.enters.size + this.overs.size + this.drops.size + this.leaves.size + this.ends.size;
	}

	private keys(given: Partial<Modifiers> | undefined): Modifiers {
		// Where the modifier keys are unavailable the plugin reports them all released.
		return this.availability.modifiers ? { ...NO_KEYS, ...given } : { ...NO_KEYS };
	}
}

function pathOf(uri: string): string {
	const text = uri.replace(/^file:\/\//, '');
	try {
		return decodeURIComponent(text);
	} catch {
		return text;
	}
}

/** A client for a system where the plugin offers nothing. */
export function unavailableNativeDnd(reason = 'no display'): FakeNativeDndClient {
	return new FakeNativeDndClient({
		availability: {
			...NO_NATIVE_DND,
			reasons: { inbound: reason, outbound: reason, modifiers: reason, clipboard: reason },
		},
	});
}
