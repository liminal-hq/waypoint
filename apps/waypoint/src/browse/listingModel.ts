// The frontend's cache of one open listing: pages fetched on demand, patched in place as it changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { Filter } from '@liminal-hq/waypoint-protocol/generated/Filter';
import type { GroupRun } from '@liminal-hq/waypoint-protocol/generated/GroupRun';
import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { ListingLayout } from '@liminal-hq/waypoint-protocol/generated/ListingLayout';
import type { ListingPhase } from '@liminal-hq/waypoint-protocol/generated/ListingPhase';
import type { ListingSnapshot } from '@liminal-hq/waypoint-protocol/generated/ListingSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { PatchOp } from '@liminal-hq/waypoint-protocol/generated/PatchOp';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { isVfsError, type OpenOptions, type VfsClient } from '../services/vfsClient';

/** Rows per fetch. The spike measured a page of this size at a few milliseconds over the IPC. */
export const PAGE_SIZE = 256;

/** Pages kept on each side of the viewport once the cache outgrows `MAX_CACHED_ENTRIES`. */
const KEEP_PAGES = 8;
const MAX_CACHED_ENTRIES = 40 * PAGE_SIZE;

/** What a patch did, for the things that keep view positions and ids of their own. */
export interface PatchReport {
	ops: PatchOp[];
	/** Ids of removed entries the cache knew. Removed entries it had not loaded cannot be named. */
	removedIds: EntryId[];
	count: number;
}

/** Coerces anything a client rejects with into a `VfsError`. */
export function toVfsError(value: unknown): VfsError {
	if (isVfsError(value)) return value;
	return {
		kind: 'io',
		message: value instanceof Error ? value.message : String(value),
		location: null,
	};
}

/**
 * Holds what the view knows of one listing: its count and sort, and the entries of the pages near
 * the viewport, keyed by view position. Rust owns the listing (A3), so every method here either
 * reads the cache or asks the client for a page.
 *
 * Consistency rests on the revision. Every reply and event carries one, and anything older than
 * the model's own is dropped: a page fetched before a patch is refetched, never merged. Entries the
 * cache cannot vouch for (after an update, a reset or a re-sort) are marked stale, not removed, so
 * the view keeps drawing them until the fresh page lands and never flashes blank.
 */
export class ListingModel {
	readonly handle: ListingHandle;
	readonly location: Location;

	private _count: number;
	private _revision: number;
	private _phase: ListingPhase;
	private _scanned: number;
	private _sort: SortSpec;
	private _filter: Filter;
	private _groups: GroupRun[];
	private _error: VfsError | null = null;
	/** What the listing's provider offers, which never changes for an open listing. */
	readonly readOnly: boolean;
	/** The provider writes nothing here but a change is made by rewriting the file that holds it: an archive that can be written (D170). */
	readonly rewritable: boolean;
	readonly layout: ListingLayout;

	private entries = new Map<number, Entry>();
	private stale = new Set<number>();
	private inflight = new Map<number, { revision: number; promise: Promise<void> }>();
	private wanted: [number, number] | null = null;

	private version = 0;
	private disposed = false;
	private listeners = new Set<() => void>();
	private patchListeners = new Set<(report: PatchReport) => void>();

	constructor(
		private client: VfsClient,
		snapshot: ListingSnapshot,
		private detach: () => void = () => {},
	) {
		this.handle = snapshot.handle;
		this.location = snapshot.location;
		this._count = snapshot.count;
		this._revision = snapshot.revision;
		this._phase = snapshot.phase;
		this._scanned = snapshot.count;
		this._sort = snapshot.sort;
		this._filter = snapshot.filter;
		this._groups = snapshot.groups;
		this.readOnly = snapshot.readOnly;
		this.rewritable = snapshot.rewritable === true;
		this.layout = snapshot.layout;
	}

	/** Whether nothing can be dropped, pasted, created or renamed here: read only, and not an archive that can be rewritten. */
	get blocksWrites(): boolean {
		return this.readOnly && !this.rewritable;
	}
	get count(): number {
		return this._count;
	}
	get revision(): number {
		return this._revision;
	}
	get phase(): ListingPhase {
		return this._phase;
	}
	get scanned(): number {
		return this._scanned;
	}
	get sort(): SortSpec {
		return this._sort;
	}
	get filter(): Filter {
		return this._filter;
	}
	/** The groups of the view in order, each a contiguous run of positions; empty when the sort does not group. */
	get groups(): readonly GroupRun[] {
		return this._groups;
	}
	get error(): VfsError | null {
		return this._error;
	}

	/** A number that changes whenever anything a view reads has; the store snapshot for React. */
	getVersion = (): number => this.version;

	subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	/** Hears each patch (and each re-sort, as a reset) synchronously, before any view re-renders. */
	onPatch(listener: (report: PatchReport) => void): () => void {
		this.patchListeners.add(listener);
		return () => {
			this.patchListeners.delete(listener);
		};
	}

	/** The entry at a view position if the cache holds one, stale or not. */
	entryAt(position: number): Entry | undefined {
		return position < this._count ? this.entries.get(position) : undefined;
	}

	/** Whether the cache holds an entry at this position that it does not doubt. */
	hasFresh(position: number): boolean {
		return this.entries.has(position) && !this.stale.has(position);
	}

	/** How many entries the cache holds, for tests and diagnostics. */
	get cachedCount(): number {
		return this.entries.size;
	}

	/** Positions the cache holds stale entries for, for tests and diagnostics. */
	get staleCount(): number {
		return this.stale.size;
	}

	/**
	 * Says which positions the view wants. Fetches the pages they touch plus one either side, unless
	 * a page is fresh or already on its way for this revision.
	 */
	ensure(first: number, last: number): void {
		if (this.disposed) return;
		this.wanted = [first, last];
		if (this._count === 0) return;
		const lastPage = Math.floor((this._count - 1) / PAGE_SIZE);
		const from = Math.max(0, Math.floor(first / PAGE_SIZE) - 1);
		const to = Math.min(lastPage, Math.floor(last / PAGE_SIZE) + 1);
		for (let page = from; page <= to; page++) {
			if (this.needsFetch(page)) void this.fetchPage(page);
		}
		this.evictFar(from, to);
	}

	/**
	 * Reads entries `[from, to)` by view position, fetching whatever pages are missing. The result is
	 * shorter than asked only if the listing shrinks or fails meanwhile.
	 */
	async readRange(from: number, to: number): Promise<Entry[]> {
		const end = Math.min(to, this._count);
		const start = Math.max(0, from);
		for (let page = Math.floor(start / PAGE_SIZE); page * PAGE_SIZE < end; page++) {
			for (let attempt = 0; attempt < 5 && this.needsFetch(page); attempt++) {
				await this.fetchPage(page);
			}
		}
		const found: Entry[] = [];
		for (let position = start; position < Math.min(end, this._count); position++) {
			const entry = this.entries.get(position);
			if (entry) found.push(entry);
		}
		return found;
	}

	/** The id of the entry at a position, fetching its page if needed. */
	async idAt(position: number): Promise<EntryId | undefined> {
		const cached = this.hasFresh(position) ? this.entries.get(position) : undefined;
		if (cached) return cached.id;
		return (await this.readRange(position, position + 1))[0]?.id;
	}

	/**
	 * Releases the cache of a listing nobody is looking at. Only the pages the view last asked for
	 * stay, marked stale, so the view paints them at once when it returns and replaces them as
	 * fresh ones arrive (no blank flash); everything else is dropped.
	 */
	evictCache(): void {
		const keep = this.wanted
			? [
					Math.floor(this.wanted[0] / PAGE_SIZE) * PAGE_SIZE,
					(Math.floor(this.wanted[1] / PAGE_SIZE) + 1) * PAGE_SIZE,
				]
			: [0, 0];
		for (const position of [...this.entries.keys()]) {
			if (position >= keep[0]! && position < keep[1]!) continue;
			this.entries.delete(position);
			this.stale.delete(position);
		}
		this.markAllStale();
	}

	/** Re-sorts the listing. Cached entries stay on screen, stale, until the new order arrives. */
	async setSort(sort: SortSpec): Promise<void> {
		try {
			const snapshot = await this.client.setSort(this.handle, sort);
			if (this.disposed) return;
			// The confirmed sort and filter always apply: the rows Rust now serves are in that order.
			// Revision, phase and count only move forward, so an event that landed first keeps them.
			this._sort = snapshot.sort;
			this._filter = snapshot.filter;
			if (snapshot.revision >= this._revision) {
				this._revision = snapshot.revision;
				this._phase = snapshot.phase;
				this._count = snapshot.count;
				this._groups = snapshot.groups;
			}
			this.markAllStale();
			this.dropBeyondCount();
			this.announce({ ops: [{ kind: 'reset' }], removedIds: [], count: this._count });
		} catch (error) {
			this.fail(error);
		}
	}

	/** Changes what the listing hides. Like a re-sort, the cached entries stay on screen, stale, until the new view arrives. */
	async setFilter(filter: Filter): Promise<void> {
		try {
			const snapshot = await this.client.setFilter(this.handle, filter);
			if (this.disposed || snapshot.revision < this._revision) return;
			this._sort = snapshot.sort;
			this._filter = snapshot.filter;
			this._revision = snapshot.revision;
			this._phase = snapshot.phase;
			this._count = snapshot.count;
			this._groups = snapshot.groups;
			this.markAllStale();
			this.dropBeyondCount();
			this.announce({ ops: [{ kind: 'reset' }], removedIds: [], count: this._count });
		} catch (error) {
			this.fail(error);
		}
	}

	/** Feeds one client event in. Events for other listings, and stale ones, are ignored. */
	applyEvent(event: ListingEvent): void {
		if (this.disposed || event.handle !== this.handle) return;
		switch (event.kind) {
			case 'progress': {
				if (event.revision < this._revision) return;
				const advanced = event.revision > this._revision;
				this._revision = event.revision;
				this._phase = event.phase;
				this._scanned = event.scanned;
				if (advanced) this.markAllStale();
				// A scan that has just filled the view brings its groups; progress otherwise has none.
				if (event.groups) this._groups = event.groups;
				if (event.count !== this._count) {
					this._count = event.count;
					this.dropBeyondCount();
				}
				this.touch();
				this.reensure();
				return;
			}
			case 'changed': {
				if (event.revision <= this._revision) return;
				const contiguous = event.revision === this._revision + 1;
				this._revision = event.revision;
				// Boundaries come whole with every change of a grouped view, so a gap in revisions
				// does not leave them stale.
				if (event.groups) this._groups = event.groups;
				const report = contiguous
					? this.applyOps(event.ops, event.count, event.moved)
					: this.resetTo(event.count);
				this.announce(report);
				this.reensure();
				return;
			}
			case 'failed':
				this._error = event.error;
				this._phase = 'failed';
				this.touch();
				return;
		}
	}

	/** Releases the listing in the client and stops listening. The model is unusable afterwards. */
	dispose(): void {
		if (this.disposed) return;
		this.disposed = true;
		this.detach();
		this.listeners.clear();
		this.patchListeners.clear();
		this.client.closeListing(this.handle).catch(() => {});
	}

	// -- cache --------------------------------------------------------------------------------

	private needsFetch(page: number): boolean {
		const end = Math.min(this._count, (page + 1) * PAGE_SIZE);
		for (let position = page * PAGE_SIZE; position < end; position++) {
			if (!this.entries.has(position) || this.stale.has(position)) return true;
		}
		return false;
	}

	private fetchPage(page: number): Promise<void> {
		const existing = this.inflight.get(page);
		if (existing && existing.revision === this._revision) return existing.promise;
		const revision = this._revision;
		const start = page * PAGE_SIZE;
		const slot = { revision, promise: Promise.resolve() };
		slot.promise = (async () => {
			let dropped = false;
			try {
				const rows = await this.client.getRange(this.handle, start, PAGE_SIZE);
				if (this.disposed) return;
				if (revision !== this._revision) {
					// The listing moved on while this page was in flight: its rows may be out of
					// position, so they are never merged.
					dropped = true;
					return;
				}
				rows.forEach((row, offset) => {
					this.entries.set(start + offset, row);
					this.stale.delete(start + offset);
				});
				this.touch();
			} catch (error) {
				if (!this.disposed && revision === this._revision) this.fail(error);
			} finally {
				if (this.inflight.get(page) === slot) this.inflight.delete(page);
				if (dropped) this.reensure();
			}
		})();
		this.inflight.set(page, slot);
		return slot.promise;
	}

	private reensure(): void {
		if (this.wanted) this.ensure(this.wanted[0], this.wanted[1]);
	}

	private evictFar(keepFromPage: number, keepToPage: number): void {
		if (this.entries.size <= MAX_CACHED_ENTRIES) return;
		const lowest = (keepFromPage - KEEP_PAGES) * PAGE_SIZE;
		const highest = (keepToPage + KEEP_PAGES + 1) * PAGE_SIZE;
		for (const position of [...this.entries.keys()]) {
			if (position >= lowest && position < highest) continue;
			this.entries.delete(position);
			this.stale.delete(position);
		}
	}

	private markAllStale(): void {
		for (const position of this.entries.keys()) this.stale.add(position);
	}

	private dropBeyondCount(): void {
		for (const position of [...this.entries.keys()]) {
			if (position < this._count) continue;
			this.entries.delete(position);
			this.stale.delete(position);
		}
	}

	private remap(move: (position: number) => number | null): void {
		const entries = new Map<number, Entry>();
		for (const [position, entry] of this.entries) {
			const next = move(position);
			if (next !== null) entries.set(next, entry);
		}
		const stale = new Set<number>();
		for (const position of this.stale) {
			const next = move(position);
			if (next !== null) stale.add(next);
		}
		this.entries = entries;
		this.stale = stale;
	}

	private resetTo(count: number): PatchReport {
		this._count = count;
		this.markAllStale();
		this.dropBeyondCount();
		return { ops: [{ kind: 'reset' }], removedIds: [], count };
	}

	/**
	 * Applies a patch to the cache in the documented order: each op's positions are in the
	 * coordinates the ops before it left. Inserted positions are simply absent (fetched next);
	 * updated ones stay in place, stale; a removal or insertion shifts the entries after it.
	 */
	private applyOps(ops: PatchOp[], count: number, moved: readonly EntryId[]): PatchReport {
		let length = this._count;
		const removedIds: EntryId[] = [];
		for (const op of ops) {
			switch (op.kind) {
				case 'remove': {
					for (let position = op.at; position < op.at + op.count; position++) {
						const entry = this.entries.get(position);
						if (entry) removedIds.push(entry.id);
					}
					this.remap((position) =>
						position < op.at ? position : position < op.at + op.count ? null : position - op.count,
					);
					length -= op.count;
					break;
				}
				case 'insert':
					this.remap((position) => (position >= op.at ? position + op.count : position));
					length += op.count;
					break;
				case 'update':
					for (let position = op.at; position < op.at + op.count; position++) {
						if (this.entries.has(position)) this.stale.add(position);
					}
					break;
				case 'reset':
					this.markAllStale();
					length = count;
					break;
			}
		}
		this._count = count;
		// A patch whose arithmetic does not reach the announced count cannot be trusted: fall back
		// to refetching rather than showing a view that disagrees with Rust.
		if (length !== count) this.markAllStale();
		this.dropBeyondCount();
		// A moved entry was removed and inserted again under the same id: it is not gone, so whatever
		// the view holds by id (the selection) stays.
		const gone = moved.length === 0 ? removedIds : removedIds.filter((id) => !moved.includes(id));
		return { ops, removedIds: gone, count };
	}

	private fail(error: unknown): void {
		const vfsError = toVfsError(error);
		// A closed or superseded listing is expected during teardown, not something to show.
		if (vfsError.kind === 'staleHandle' || vfsError.kind === 'cancelled') return;
		this._error = vfsError;
		this.touch();
	}

	private announce(report: PatchReport): void {
		for (const listener of [...this.patchListeners]) listener(report);
		this.touch();
	}

	private touch(): void {
		this.version += 1;
		for (const listener of [...this.listeners]) listener();
	}
}

/**
 * Opens a listing and returns its model. Events that arrive between the client handing out the
 * handle and this function returning are held and replayed, so nothing is lost to the gap.
 */
export async function openListingModel(
	client: VfsClient,
	location: Location,
	options?: OpenOptions,
): Promise<ListingModel> {
	const held: ListingEvent[] = [];
	let model: ListingModel | null = null;
	const detach = client.onListingEvent((event) => {
		if (model) model.applyEvent(event);
		else held.push(event);
	});
	try {
		const snapshot = await client.openListing(location, options);
		model = new ListingModel(client, snapshot, detach);
		for (const event of held) model.applyEvent(event);
		return model;
	} catch (error) {
		detach();
		throw error;
	}
}
