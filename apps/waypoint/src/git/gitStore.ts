// The repositories this window is watching: one entry per folder shown, shared by every part of the window that reads it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GitSummary } from '@liminal-hq/waypoint-protocol/generated/GitSummary';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { GitClient } from '../services/gitClient';

/** A repository a folder is in, and how it stands. */
export interface Repository {
	/** The plugin's watch, for events and for stopping it. */
	id: number;
	/** The repository's working folder. */
	root: Location;
	/** Its name: the last folder of its path. */
	name: string;
	/** Where `HEAD` is, the distance from the upstream and the counts; `null` until the first status is done. */
	summary: GitSummary | null;
	/** The plugin's revision of this watch: only a higher one replaces what is held. */
	revision: number;
}

interface Entry {
	location: Location;
	refs: number;
	/** `null` while the plugin has not answered, and for a folder that is in no repository. */
	repository: Repository | null;
	/** Whether the plugin has answered. */
	known: boolean;
}

/**
 * Follows the Git plugin for the folders a window shows. `acquire` starts watching a folder's
 * repository (once however many parts ask) and the returned function lets go; the plugin's events
 * keep each repository's summary current. Rust owns the state: this only holds what it said last.
 * The same snapshot object is returned until something changes, so a component can read it with
 * `useSyncExternalStore`.
 */
export class GitStore {
	private readonly entries = new Map<string, Entry>();
	private readonly listeners = new Set<() => void>();
	private version = 0;
	private readonly stop: () => void;
	private disposed = false;

	constructor(private readonly client: GitClient) {
		this.stop = client.onChanged((changed) => {
			for (const entry of this.entries.values()) {
				const repository = entry.repository;
				if (!repository || repository.id !== changed.id) continue;
				if (changed.revision <= repository.revision) continue;
				entry.repository = {
					...repository,
					summary: changed.summary,
					revision: changed.revision,
				};
				this.notify();
			}
		});
	}

	/** Starts watching the repository of `location`; call the result to stop caring about it. */
	acquire(location: Location): () => void {
		if (this.disposed) return () => {};
		let entry = this.entries.get(location.uri);
		if (!entry) {
			entry = { location, refs: 0, repository: null, known: false };
			this.entries.set(location.uri, entry);
			this.open(entry);
		}
		entry.refs += 1;
		let released = false;
		return () => {
			if (released) return;
			released = true;
			this.release(location.uri);
		};
	}

	private open(entry: Entry) {
		void this.client.watch(entry.location).then(
			(watch) => {
				const live = this.entries.get(entry.location.uri) === entry && !this.disposed;
				if (!watch) {
					if (live) {
						entry.known = true;
						this.notify();
					}
					return;
				}
				if (!live) {
					void this.client.unwatch(watch.id);
					return;
				}
				entry.known = true;
				// An event that beat the reply carries a higher revision than the reply's.
				entry.repository = {
					id: watch.id,
					root: watch.root,
					name: watch.name,
					summary: watch.summary,
					revision: watch.revision,
				};
				this.notify();
			},
			(error: unknown) => {
				console.warn('could not watch the Git status of a folder', error);
				if (this.entries.get(entry.location.uri) === entry) {
					entry.known = true;
					this.notify();
				}
			},
		);
	}

	private release(uri: string) {
		const entry = this.entries.get(uri);
		if (!entry) return;
		entry.refs -= 1;
		if (entry.refs > 0) return;
		this.entries.delete(uri);
		if (entry.repository) void this.client.unwatch(entry.repository.id);
	}

	/** The repository `location` is in: `null` when it is in none, or before the plugin has said. */
	repository(location: Location): Repository | null {
		return this.entries.get(location.uri)?.repository ?? null;
	}

	/** Whether the plugin has answered for `location`. */
	known(location: Location): boolean {
		return this.entries.get(location.uri)?.known ?? false;
	}

	/** How many folders are being watched. */
	get watching(): number {
		return this.entries.size;
	}

	subscribe = (listener: () => void): (() => void) => {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	getVersion = (): number => this.version;

	private notify() {
		this.version += 1;
		for (const listener of [...this.listeners]) listener();
	}

	/** Stops listening and lets go of every watch. */
	dispose() {
		if (this.disposed) return;
		this.disposed = true;
		this.stop();
		for (const entry of this.entries.values()) {
			if (entry.repository) void this.client.unwatch(entry.repository.id);
		}
		this.entries.clear();
		this.listeners.clear();
	}
}
