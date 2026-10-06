// Owns one listing per tab: opens the folder of every tab on screen, closes it when the tab leaves it or closes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { OpenOptions, RefreshOptions, VfsClient } from '../services/vfsClient';
import { schemeOfKey } from '../connections/connectionsModel';
import { isOverviewLocation } from '../overview/overviewLocation';
import { openListingModel, toVfsError, type ListingModel } from './listingModel';
import { applyHints } from './tabHints';
import { createListingSession, type ListingSession, type SessionState } from './useListingSession';
import { sameSort, type ViewMode } from './viewStore';

/** How long a tab can be in the background before it drops the pages it has cached. */
export const BACKGROUND_EVICT_DELAY_MS = 15_000;

/** A folder nothing watches is read again when it is shown or its window is focused after this long (D150). */
export const STALE_AFTER_MS = 10_000;

/** A released hold keeps its listing this long, so a tab springing back to it finds the very session. */
export const RETAIN_GRACE_MS = 1000;

interface Held {
	tab: TabId;
	uri: string;
	holds: number;
	/** Set while nobody holds the listing and it waits out the grace period. */
	timer: ReturnType<typeof setTimeout> | null;
}

export interface ListingManagerOptions {
	/** What a newly opened listing of `location` starts with; `inherited` is the sort of the listing it replaces (used as is when omitted). */
	openOptions?: (inherited: SortSpec | undefined, location: Location) => OpenOptions;
	evictDelayMs?: number;
	/** How long a listing nobody holds any more is kept for the tab that left it to come back (`retain`). */
	retainGraceMs?: number;
	/**
	 * Hears the sort a folder listing of `location` now has whenever it changes (the person chose
	 * another sort or grouping), so the window can remember it, for the folders it opens next or for
	 * this folder. The Trash is not a folder listing: its columns are its own.
	 */
	onSort?: (sort: SortSpec, location: Location) => void;
	/** The layout in use, which decides which scroll offset a restored tab's hint belongs to. */
	viewMode?: () => ViewMode;
}

interface Slot {
	uri: string;
	state: SessionState;
	evictTimer: ReturnType<typeof setTimeout> | null;
	evicted: boolean;
	/** Set while the tab is not on screen, so a listing that finishes opening in the background still evicts. */
	background: boolean;
	/** The hidden-files choice last sent to this listing, which its confirmed filter may not reflect yet. */
	requestedHidden?: boolean;
}

/**
 * A listing is a handle in Rust and memory in the webview, so it lives only as long as a tab needs
 * it (A9, A20). The listing of every tab on screen (the active tab, or all panes of its pair) opens when the tab shows a folder and closes when the tab
 * navigates elsewhere or closes. A background tab keeps its listing for a short while, then drops
 * its cached pages down to the ones it last showed (stale, so they still paint on return while
 * fresh ones load) and, on a location change made while hidden, closes it outright.
 *
 * `sync` is idempotent: call it with every session snapshot and the manager converges on it.
 */
export class ListingManager {
	private slots = new Map<TabId, Slot>();
	private listeners = new Set<() => void>();
	private version = 0;
	/** Tabs whose hints (restored scroll and focus) have been applied: they apply once, to the first listing. */
	private hinted = new Set<TabId>();
	/** The latest hidden-files choice, applied to listings that become ready after it was made. */
	private wantedHidden: boolean | null = null;
	/** The tabs of the last `sync`, so a failed listing can be opened again. */
	private tabs = new Map<TabId, TabSnapshot>();
	/** Sessions someone holds on to (a file drag's sources), by session, with how many hold each. */
	private retained = new Map<ListingSession, Held>();

	constructor(
		private client: VfsClient,
		private options: ListingManagerOptions = {},
	) {}

	getVersion = (): number => this.version;

	subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	/** What the tab's view shows: opening, the listing, or why it could not open. */
	stateFor(tab: TabId): SessionState | undefined {
		return this.slots.get(tab)?.state;
	}

	/**
	 * Brings the open listings in line with the tabs and the ones on screen: the active tab, or
	 * every pane of the active tab's pair. Each visible tab has a live listing of its own.
	 */
	sync(tabs: readonly TabSnapshot[], visible: ReadonlySet<TabId>): void {
		this.tabs = new Map(tabs.map((tab) => [tab.id, tab]));
		const live = new Set(tabs.map((tab) => tab.id));
		for (const id of [...this.slots.keys()]) {
			if (!live.has(id)) this.release(id);
		}
		for (const id of [...this.hinted]) {
			if (!live.has(id)) this.hinted.delete(id);
		}
		for (const tab of tabs) {
			const slot = this.slots.get(tab.id);
			// Overview is a page, not a folder: a tab on it holds no listing, and one that moved
			// onto it lets go of the folder it had.
			if (isOverviewLocation(tab.location)) {
				if (slot) this.release(tab.id);
				continue;
			}
			if (visible.has(tab.id)) {
				this.stopEvicting(slot);
				const wasHidden = slot?.background === true;
				if (slot) slot.background = false;
				if (!slot || slot.uri !== tab.location.uri) this.open(tab);
				else {
					if (slot.evicted) slot.evicted = false;
					// Shown again after a while in the background: what a folder nothing watches
					// held may be out of date (a watched one has been kept current all along).
					if (wasHidden) this.refreshSlot(slot, { onlyUnwatched: true, minAgeMs: STALE_AFTER_MS });
				}
			} else if (slot) {
				slot.background = true;
				if (slot.uri !== tab.location.uri) this.release(tab.id);
				else this.scheduleEviction(slot);
			}
		}
	}

	/**
	 * Keeps the listing a tab shows open until the returned function is called, even when the tab
	 * navigates away: a file drag's sources are a handle into that listing, and spring-loading a
	 * folder navigates the pane the drag began in. When the tab comes back to the same folder during
	 * the hold, or just after it (`retainGraceMs`), it gets the very session back, its selection and
	 * scroll with it, so a drag that is cancelled leaves the pane as it was. Returns `null` when
	 * the tab has no ready listing.
	 */
	retain(tab: TabId): (() => void) | null {
		const slot = this.slots.get(tab);
		if (!slot || slot.state.status !== 'ready') return null;
		const session = slot.state.session;
		const held = this.retained.get(session) ?? { tab, uri: slot.uri, holds: 0, timer: null };
		if (held.timer) clearTimeout(held.timer);
		held.timer = null;
		held.holds += 1;
		this.retained.set(session, held);
		let released = false;
		return () => {
			if (released) return;
			released = true;
			held.holds -= 1;
			if (held.holds > 0) return;
			if (this.isLive(held.tab, session)) {
				this.retained.delete(session);
				return;
			}
			// Not the tab's listing now: it is kept a moment longer, in case the tab is on its way back.
			held.timer = setTimeout(() => {
				held.timer = null;
				this.retained.delete(session);
				if (!this.isLive(held.tab, session)) session.model.dispose();
			}, this.options.retainGraceMs ?? RETAIN_GRACE_MS);
		};
	}

	private isLive(tab: TabId, session: ListingSession): boolean {
		const current = this.slots.get(tab)?.state;
		return current?.status === 'ready' && current.session === session;
	}

	/**
	 * Gives every open folder listing the sort `wanted` says for its location (`undefined` leaves
	 * it as it is). A listing that already has it is not touched. The sorts change through the
	 * listing's own `setSort`, which `onSort` hears like any other change.
	 */
	applySorts(wanted: (location: Location) => SortSpec | undefined): void {
		for (const slot of this.slots.values()) {
			if (slot.state.status !== 'ready') continue;
			const { model } = slot.state.session;
			if (model.layout !== 'folder') continue;
			const sort = wanted(model.location);
			if (sort && !sameSort(sort, model.sort)) void model.setSort(sort);
		}
	}

	/**
	 * Reads again the folder of every listing on screen that nothing watches and that was read more
	 * than `STALE_AFTER_MS` ago: the window was focused (D150). A watched folder is already current.
	 */
	refreshShown(): void {
		for (const slot of this.slots.values()) {
			if (!slot.background) {
				this.refreshSlot(slot, { onlyUnwatched: true, minAgeMs: STALE_AFTER_MS });
			}
		}
	}

	/**
	 * Reads the folder of the listing on screen again whatever its age: the person asked for a
	 * refresh. Every pane on screen refreshes, a watched folder too (a cheap, harmless read).
	 */
	refreshNow(tab?: TabId): void {
		for (const [id, slot] of this.slots) {
			if (slot.background || (tab !== undefined && id !== tab)) continue;
			this.refreshSlot(slot, {});
		}
	}

	/**
	 * Reads again every unwatched folder that is open, on screen or not: Waypoint wrote to the
	 * file system (a job finished), and a server would not say what changed.
	 */
	refreshAfterWrite(): void {
		for (const slot of this.slots.values()) this.refreshSlot(slot, { onlyUnwatched: true });
	}

	private refreshSlot(slot: Slot, options: RefreshOptions): void {
		if (slot.state.status !== 'ready') return;
		void slot.state.session.model.refresh(options);
	}

	/** Applies the hidden-files choice to every open listing; listings opened later take it from `openOptions`. */
	setShowHidden(showHidden: boolean): void {
		this.wantedHidden = showHidden;
		for (const slot of this.slots.values()) this.applyHidden(slot);
	}

	// Compares with the last request, not the confirmed filter, so the last choice always wins.
	private applyHidden(slot: Slot): void {
		if (this.wantedHidden === null || slot.state.status !== 'ready') return;
		const { model } = slot.state.session;
		const current = slot.requestedHidden ?? model.filter.showHidden;
		if (current === this.wantedHidden) return;
		slot.requestedHidden = this.wantedHidden;
		void model.setFilter({ showHidden: this.wantedHidden });
	}

	/**
	 * Opens again every listing on screen that could not open, or failed once open, with an error
	 * `retry` accepts (a server's connection that is back). The others are left as they are. Returns
	 * how many were opened again.
	 */
	retryFailed(retry: (error: VfsError) => boolean): number {
		let count = 0;
		for (const [id, slot] of [...this.slots]) {
			if (slot.background) continue;
			const error =
				slot.state.status === 'error'
					? slot.state.error
					: slot.state.status === 'ready'
						? slot.state.session.model.error
						: null;
			const tab = this.tabs.get(id);
			if (!error || !tab || tab.location.uri !== slot.uri || !retry(error)) continue;
			this.open(tab);
			count += 1;
		}
		return count;
	}

	/**
	 * Settings → Experimental turned remote protocols off: every listing on one of `off` closes at
	 * once (so nothing watches, polls or refreshes it) and its tab shows the protocol's typed
	 * turned-off state, `ProtocolOffState`, with the link to switch it back on. A listing that was
	 * showing the turned-off state of a protocol that is no longer in `off` opens again. Returns how
	 * many tabs changed.
	 */
	applyProtocols(off: readonly string[]): number {
		const turnedOff = new Set(off.map((scheme) => scheme.toLowerCase()));
		let count = 0;
		for (const [id, slot] of [...this.slots]) {
			const scheme = schemeOfKey(slot.uri);
			const showingOff = slot.state.status === 'error' && slot.state.error.kind === 'protocolOff';
			if (turnedOff.has(scheme) && !showingOff) {
				const state = slot.state;
				if (state.status === 'ready' && !this.retained.has(state.session)) {
					state.session.model.dispose();
				}
				slot.state = { status: 'error', error: { kind: 'protocolOff', scheme } };
				this.stopEvicting(slot);
				count += 1;
			} else if (!turnedOff.has(scheme) && showingOff) {
				const tab = this.tabs.get(id);
				if (!tab || tab.location.uri !== slot.uri) continue;
				// A tab in the background opens when it is shown again.
				if (slot.background) this.release(id);
				else this.open(tab);
				count += 1;
			}
		}
		if (count > 0) this.changed();
		return count;
	}

	/** Closes every listing. The manager can be used again: the next `sync` reopens what is shown. */
	dispose(): void {
		for (const id of [...this.slots.keys()]) this.release(id);
		// Nothing is left to come back to: what was only being kept goes too.
		for (const [session, held] of [...this.retained]) {
			if (held.timer) clearTimeout(held.timer);
			this.retained.delete(session);
			session.model.dispose();
		}
	}

	/** How many tabs hold a listing, for tests. */
	get openCount(): number {
		return this.slots.size;
	}

	private open(tab: TabSnapshot): void {
		const previous = this.slots.get(tab.id);
		const inherited =
			previous?.state.status === 'ready' ? previous.state.session.model.sort : undefined;
		if (previous) {
			this.release(tab.id);
			// The tab moved on from the listing its hints described.
			this.hinted.add(tab.id);
		}

		// A drag still holds the session of this very folder (the pane was sprung away and came back).
		for (const [session, held] of this.retained) {
			if (held.tab !== tab.id || held.uri !== tab.location.uri) continue;
			// Adopted: the hold, if any is left, now only guards a listing that is the tab's own.
			if (held.timer) clearTimeout(held.timer);
			held.timer = null;
			if (held.holds === 0) this.retained.delete(session);
			this.slots.set(tab.id, {
				uri: tab.location.uri,
				state: { status: 'ready', session },
				evictTimer: null,
				evicted: false,
				background: false,
			});
			this.changed();
			return;
		}

		const slot: Slot = {
			uri: tab.location.uri,
			state: { status: 'opening' },
			evictTimer: null,
			evicted: false,
			background: false,
		};
		this.slots.set(tab.id, slot);
		this.changed();

		const options: OpenOptions | undefined =
			this.options.openOptions?.(inherited, tab.location) ??
			(inherited ? { sort: inherited } : undefined);
		openListingModel(this.client, tab.location, options).then(
			(model) => {
				// The tab moved on (or closed) while the listing was opening: nobody wants it.
				if (this.slots.get(tab.id) !== slot) {
					model.dispose();
					return;
				}
				const session = createListingSession(model);
				slot.state = { status: 'ready', session };
				this.followSort(model, tab.location);
				if (!this.hinted.has(tab.id)) {
					this.hinted.add(tab.id);
					applyHints(session, tab.hints, this.options.viewMode?.() ?? 'list');
				}
				if (slot.background) this.scheduleEviction(slot);
				// A toggle made while this listing was opening has not reached it yet.
				this.applyHidden(slot);
				this.changed();
			},
			(error: unknown) => {
				if (this.slots.get(tab.id) !== slot) return;
				slot.state = { status: 'error', error: toVfsError(error) };
				this.changed();
			},
		);
	}

	/** Reports each change of a folder listing's sort to `onSort`. */
	private followSort(model: ListingModel, location: Location): void {
		if (model.layout !== 'folder' || !this.options.onSort) return;
		let last = model.sort;
		model.subscribe(() => {
			const now = model.sort;
			if (sameSort(now, last)) return;
			last = now;
			this.options.onSort?.(now, location);
		});
	}

	private release(tab: TabId): void {
		const slot = this.slots.get(tab);
		if (!slot) return;
		this.stopEvicting(slot);
		this.slots.delete(tab);
		if (slot.state.status === 'ready' && !this.retained.has(slot.state.session)) {
			slot.state.session.model.dispose();
		}
		this.changed();
	}

	private scheduleEviction(slot: Slot): void {
		if (slot.evictTimer || slot.evicted || slot.state.status !== 'ready') return;
		slot.evictTimer = setTimeout(() => {
			slot.evictTimer = null;
			slot.evicted = true;
			if (slot.state.status === 'ready') slot.state.session.model.evictCache();
		}, this.options.evictDelayMs ?? BACKGROUND_EVICT_DELAY_MS);
	}

	private stopEvicting(slot: Slot | undefined): void {
		if (slot?.evictTimer) clearTimeout(slot.evictTimer);
		if (slot) slot.evictTimer = null;
	}

	private changed(): void {
		this.version += 1;
		for (const listener of [...this.listeners]) listener();
	}
}
