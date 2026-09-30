// Owns one listing per tab: opens the active tab's folder, closes it when the tab leaves it or closes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { OpenOptions, VfsClient } from '../services/vfsClient';
import { openListingModel, toVfsError } from './listingModel';
import { createListingSession, type SessionState } from './useListingSession';

/** How long a tab can be in the background before it drops the pages it has cached. */
export const BACKGROUND_EVICT_DELAY_MS = 15_000;

export interface ListingManagerOptions {
	/** What a newly opened listing starts with; `inherited` is the sort of the listing it replaces (used as is when omitted). */
	openOptions?: (inherited: SortSpec | undefined) => OpenOptions;
	evictDelayMs?: number;
}

interface Slot {
	uri: string;
	state: SessionState;
	evictTimer: ReturnType<typeof setTimeout> | null;
	evicted: boolean;
	/** Set while the tab is not the active one, so a listing that finishes opening in the background still evicts. */
	background: boolean;
}

/**
 * A listing is a handle in Rust and memory in the webview, so it lives only as long as a tab needs
 * it (A9, A20). The active tab's listing opens when the tab shows a folder and closes when the tab
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

	/** Brings the open listings in line with the tabs and which one is active. */
	sync(tabs: readonly TabSnapshot[], active: TabId | null): void {
		const live = new Set(tabs.map((tab) => tab.id));
		for (const id of [...this.slots.keys()]) {
			if (!live.has(id)) this.release(id);
		}
		for (const tab of tabs) {
			const slot = this.slots.get(tab.id);
			if (tab.id === active) {
				this.stopEvicting(slot);
				if (slot) slot.background = false;
				if (!slot || slot.uri !== tab.location.uri) this.open(tab);
				else if (slot.evicted) slot.evicted = false;
			} else if (slot) {
				slot.background = true;
				if (slot.uri !== tab.location.uri) this.release(tab.id);
				else this.scheduleEviction(slot);
			}
		}
	}

	/** Closes every listing. The manager can be used again: the next `sync` reopens what is shown. */
	dispose(): void {
		for (const id of [...this.slots.keys()]) this.release(id);
	}

	/** How many tabs hold a listing, for tests. */
	get openCount(): number {
		return this.slots.size;
	}

	private open(tab: TabSnapshot): void {
		const previous = this.slots.get(tab.id);
		const inherited =
			previous?.state.status === 'ready' ? previous.state.session.model.sort : undefined;
		if (previous) this.release(tab.id);

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
			this.options.openOptions?.(inherited) ?? (inherited ? { sort: inherited } : undefined);
		openListingModel(this.client, tab.location, options).then(
			(model) => {
				// The tab moved on (or closed) while the listing was opening: nobody wants it.
				if (this.slots.get(tab.id) !== slot) {
					model.dispose();
					return;
				}
				slot.state = { status: 'ready', session: createListingSession(model) };
				if (slot.background) this.scheduleEviction(slot);
				this.changed();
			},
			(error: unknown) => {
				if (this.slots.get(tab.id) !== slot) return;
				slot.state = { status: 'error', error: toVfsError(error) };
				this.changed();
			},
		);
	}

	private release(tab: TabId): void {
		const slot = this.slots.get(tab);
		if (!slot) return;
		this.stopEvicting(slot);
		this.slots.delete(tab);
		if (slot.state.status === 'ready') slot.state.session.model.dispose();
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
