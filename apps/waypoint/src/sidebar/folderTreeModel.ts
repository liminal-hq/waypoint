// Loads the children of the Folders tree's expanded nodes, one folders-only listing per expanded node
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { openListingModel, toVfsError, type ListingModel } from '../browse/listingModel';
import type { VfsClient } from '../services/vfsClient';

/** The most children one node shows; a folder with more shows the first of them and says so. */
export const MAX_CHILDREN = 2000;

export interface FolderChild {
	name: string;
	location: Location;
}

export type ChildrenState =
	| { status: 'loading' }
	| { status: 'ready'; children: FolderChild[]; total: number }
	| { status: 'error' };

interface Slot {
	location: Location;
	state: ChildrenState;
	model: ListingModel | null;
	/** The revision and phase the children were last read at, so an unrelated model change does not reload them. */
	loadedKey: string | null;
	loadToken: number;
	unsubscribe: (() => void) | null;
}

/**
 * The Folders tree is lazy (SPEC 5.4): a folder's children are read only while it is expanded,
 * through a listing that keeps folders alone and follows the window's hidden-files choice. The
 * listing stays open while the node is expanded, so a folder created or removed underneath shows
 * up without a refresh, and is closed when the node collapses or the model is disposed (A9).
 *
 * `sync` is idempotent: call it with the locations that should be loaded and the model converges.
 */
export class FolderTreeModel {
	private slots = new Map<string, Slot>();
	private listeners = new Set<() => void>();
	private version = 0;
	private showHidden = false;

	constructor(private client: VfsClient) {}

	getVersion = (): number => this.version;

	subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	/** What is known of a node's children: `undefined` when it is not being loaded. */
	childrenOf(uri: string): ChildrenState | undefined {
		return this.slots.get(uri)?.state;
	}

	/** How many nodes hold a listing open, for tests. */
	get openCount(): number {
		return this.slots.size;
	}

	/** Loads the children of `wanted`, closes the listing of every other node, and applies `showHidden`. */
	sync(wanted: readonly Location[], showHidden: boolean): void {
		const uris = new Set(wanted.map((location) => location.uri));
		for (const uri of [...this.slots.keys()]) {
			if (!uris.has(uri)) this.release(uri);
		}
		if (showHidden !== this.showHidden) {
			this.showHidden = showHidden;
			for (const slot of this.slots.values()) {
				void slot.model?.setFilter({ showHidden, only: 'directories' });
			}
		}
		for (const location of wanted) {
			if (!this.slots.has(location.uri)) this.open(location);
		}
	}

	/** Closes every listing. The model can be used again: the next `sync` reopens what is wanted. */
	dispose(): void {
		for (const uri of [...this.slots.keys()]) this.release(uri);
	}

	private open(location: Location): void {
		const slot: Slot = {
			location,
			state: { status: 'loading' },
			model: null,
			loadedKey: null,
			loadToken: 0,
			unsubscribe: null,
		};
		this.slots.set(location.uri, slot);
		this.changed();
		openListingModel(this.client, location, {
			filter: { showHidden: this.showHidden, only: 'directories' },
		}).then(
			(model) => {
				// Collapsed (or disposed) while the listing was opening: nobody wants it.
				if (this.slots.get(location.uri) !== slot) {
					model.dispose();
					return;
				}
				slot.model = model;
				slot.unsubscribe = model.subscribe(() => this.load(slot));
				// The hidden-files choice may have changed while the listing was opening.
				if (model.filter.showHidden !== this.showHidden) {
					void model.setFilter({ showHidden: this.showHidden, only: 'directories' });
				}
				this.load(slot);
			},
			(error: unknown) => {
				if (this.slots.get(location.uri) !== slot) return;
				console.warn('could not list the folders', toVfsError(error));
				slot.state = { status: 'error' };
				this.changed();
			},
		);
	}

	private release(uri: string): void {
		const slot = this.slots.get(uri);
		if (!slot) return;
		this.slots.delete(uri);
		slot.loadToken += 1;
		slot.unsubscribe?.();
		slot.model?.dispose();
		this.changed();
	}

	/** Reads the children once the listing is ready, again whenever its revision moves. */
	private load(slot: Slot): void {
		const model = slot.model;
		if (!model || this.slots.get(slot.location.uri) !== slot) return;
		if (model.error) {
			if (slot.state.status !== 'error') {
				slot.state = { status: 'error' };
				this.changed();
			}
			return;
		}
		if (model.phase !== 'ready') return;
		const key = `${model.revision}:${model.count}`;
		if (slot.loadedKey === key) return;
		slot.loadedKey = key;
		const token = ++slot.loadToken;
		const total = model.count;
		model
			.readRange(0, Math.min(total, MAX_CHILDREN))
			.then((entries) =>
				Promise.all(
					entries.map(async (entry): Promise<FolderChild> => ({
						name: entry.name,
						location: await this.client.entryLocation(model.handle, entry.id),
					})),
				),
			)
			.then(
				(children) => {
					if (slot.loadToken !== token) return;
					slot.state = { status: 'ready', children, total };
					this.changed();
				},
				(error: unknown) => {
					if (slot.loadToken !== token) return;
					// A stale handle means the listing was closed under us; anything else is shown as an error.
					console.warn('could not read the folders', toVfsError(error));
					slot.loadedKey = null;
					slot.state = { status: 'error' };
					this.changed();
				},
			);
	}

	private changed(): void {
		this.version += 1;
		for (const listener of [...this.listeners]) listener();
	}
}
