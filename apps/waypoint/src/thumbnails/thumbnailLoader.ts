// Asks for the thumbnails a view shows, keeps what arrived, and withdraws what scrolled away
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ThumbEvent, ThumbSize, Ticket } from './thumbnailsClient';

/** Pictures kept at most; the oldest finished ones go first. */
const MAX_KEPT = 4000;

export type ThumbState =
	{ status: 'pending' } | { status: 'ready'; url: string } | { status: 'none' };

/** What a loader needs from a client: one function per kind of source, so it works for entries and locations alike. */
export interface LoaderTransport<Item extends { key: string }> {
	request(items: Item[], size: ThumbSize, onEvent: (event: ThumbEvent) => void): Promise<Ticket>;
	prioritise(ticket: Ticket, keys: string[]): Promise<void>;
	cancel(ticket: Ticket): Promise<boolean>;
}

interface Batch {
	/** `null` until the client has answered. */
	ticket: Ticket | null;
	/** Keys sent that have not been answered. */
	pending: Set<string>;
	/** Withdrawn before the ticket was known. */
	withdrawn: boolean;
	/** The keys last sent to `prioritise`, so the same order is not sent twice. */
	lastPriority: string;
}

/**
 * The thumbnails of one view at one size. `want` says what is on screen (first) and what is near it;
 * the loader asks the client only for what it has not asked for, moves what is on screen to the
 * front of the queue, and withdraws a batch when nothing it holds is wanted any more. A result
 * arrives under the key it was asked for and is kept, so scrolling back shows it at once; a
 * failure or a skip is kept too, so a file with no thumbnail is not asked about again.
 */
export class ThumbnailLoader<Item extends { key: string }> {
	private readonly states = new Map<string, ThumbState>();
	private readonly listeners = new Map<string, Set<() => void>>();
	private readonly batches = new Set<Batch>();
	private disposed = false;

	constructor(
		private readonly transport: LoaderTransport<Item>,
		readonly size: ThumbSize,
	) {}

	/** What is known of `key`: nothing asked yet is `undefined`. */
	stateOf(key: string): ThumbState | undefined {
		return this.states.get(key);
	}

	/** The address of `key`'s picture, or `null` while it is pending, absent or has none. */
	urlOf(key: string): string | null {
		const state = this.states.get(key);
		return state?.status === 'ready' ? state.url : null;
	}

	/** Runs `listener` whenever `key`'s state changes; returns the way to stop. */
	subscribe(key: string, listener: () => void): () => void {
		let set = this.listeners.get(key);
		if (!set) this.listeners.set(key, (set = new Set()));
		set.add(listener);
		return () => {
			set.delete(listener);
			if (set.size === 0) this.listeners.delete(key);
		};
	}

	/**
	 * Sets what the view shows. `visible` is in view, nearest the top first; `near` is the screen
	 * either side. Items not asked for yet are queued as one batch (visible ones first); every
	 * batch with a visible item pending has it moved to the front; a batch with nothing wanted left
	 * is withdrawn, and its unanswered keys are forgotten so they can be asked for again.
	 */
	want(visible: readonly Item[], near: readonly Item[] = []): void {
		if (this.disposed) return;
		const ordered = [...visible, ...near];
		const wanted = new Set(ordered.map((item) => item.key));
		const fresh = ordered.filter((item) => !this.states.has(item.key));

		for (const batch of [...this.batches]) {
			if ([...batch.pending].some((key) => wanted.has(key))) continue;
			this.withdraw(batch);
		}
		for (const batch of this.batches) {
			if (batch.ticket === null) continue;
			const keys = visible.map((item) => item.key).filter((key) => batch.pending.has(key));
			const signature = keys.join('\u0000');
			if (keys.length === 0 || signature === batch.lastPriority) continue;
			batch.lastPriority = signature;
			void this.transport.prioritise(batch.ticket, keys).catch(() => undefined);
		}
		if (fresh.length > 0) this.send(fresh);
	}

	/** Withdraws everything and stops listening: the view is gone. */
	dispose(): void {
		this.disposed = true;
		// The view is gone, so nothing needs telling that its pictures are.
		this.listeners.clear();
		for (const batch of [...this.batches]) this.withdraw(batch);
	}

	private send(items: Item[]): void {
		const batch: Batch = {
			ticket: null,
			pending: new Set(items.map((item) => item.key)),
			withdrawn: false,
			lastPriority: '',
		};
		this.batches.add(batch);
		for (const item of items) this.states.set(item.key, { status: 'pending' });
		this.transport
			.request(items, this.size, (event) => this.receive(batch, event))
			.then(
				(ticket) => {
					batch.ticket = ticket;
					if (batch.withdrawn || this.disposed)
						void this.transport.cancel(ticket).catch(() => undefined);
				},
				() => {
					// The request itself failed: nothing will arrive, so the icons stay.
					for (const key of batch.pending) this.settle(key, { status: 'none' });
					batch.pending.clear();
					this.batches.delete(batch);
				},
			);
	}

	private receive(batch: Batch, event: ThumbEvent): void {
		if (batch.withdrawn || this.disposed || !batch.pending.delete(event.key)) return;
		this.settle(
			event.key,
			event.kind === 'ready' ? { status: 'ready', url: event.url } : { status: 'none' },
		);
		if (batch.pending.size === 0) this.batches.delete(batch);
		this.trim();
	}

	private withdraw(batch: Batch): void {
		batch.withdrawn = true;
		this.batches.delete(batch);
		if (batch.ticket !== null) void this.transport.cancel(batch.ticket).catch(() => undefined);
		for (const key of batch.pending) {
			this.states.delete(key);
			this.notify(key);
		}
		batch.pending.clear();
	}

	private settle(key: string, state: ThumbState): void {
		this.states.set(key, state);
		this.notify(key);
	}

	private notify(key: string): void {
		for (const listener of [...(this.listeners.get(key) ?? [])]) listener();
	}

	/** Lets go of the oldest finished results once there are too many. */
	private trim(): void {
		if (this.states.size <= MAX_KEPT) return;
		for (const [key, state] of this.states) {
			if (this.states.size <= MAX_KEPT) break;
			if (state.status !== 'pending' && !this.listeners.has(key)) this.states.delete(key);
		}
	}
}
