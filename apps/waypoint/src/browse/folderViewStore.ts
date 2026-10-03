// The remembered folder views as a window's copy of Rust's: one map kept current by revision-gated events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import { createStore, type StoreApi } from 'zustand/vanilla';
import type {
	FolderView,
	FolderViewsChanged,
	FolderViewsClient,
	FolderViewsSnapshot,
} from '../services/folderViewsClient';
import type { ViewMode } from './viewStore';

export interface FolderViewsState {
	/** What each remembered folder keeps, by the folder's location `uri`. */
	folders: ReadonlyMap<string, FolderView>;
	/** The revision `folders` is at; 0 until the first snapshot. */
	revision: number;
	/** Whether the first snapshot has arrived. Until then nothing is known to be remembered. */
	ready: boolean;
	/**
	 * How many of this window's writes Rust has not answered. While any is out, the window leaves
	 * the folders it shows alone: an event for the first of two quick choices must not turn the
	 * second one back.
	 */
	writing: number;
}

/** What the window chooses for a folder; a field left out is not remembered by this write. */
export interface FolderViewChoice {
	mode?: ViewMode;
	sort?: SortSpec;
}

export interface FolderViewsHandle {
	store: StoreApi<FolderViewsState>;
	/** Settles once the first snapshot is in (or could not be read, and nothing is remembered). */
	ready: Promise<void>;
	/** Asks Rust to remember `choice` for the folder at `key`. Rejects with Rust's refusal; the map changes only by what Rust announces. */
	remember(key: string, choice: FolderViewChoice): Promise<void>;
	/** Asks Rust to make the folder forget its own view. */
	reset(key: string): Promise<void>;
	/** Stops following. The store keeps what it holds. */
	dispose(): void;
}

/** The state of a window with no service: nothing is remembered and nothing is being written. */
export const IDLE_FOLDER_VIEWS: StoreApi<FolderViewsState> = createStore<FolderViewsState>()(
	() => ({
		folders: new Map(),
		revision: 0,
		ready: false,
		writing: 0,
	}),
);

/** What no folder remembers, for a window where remembering is off. */
export const NO_FOLDERS: ReadonlyMap<string, FolderView> = new Map();

function fromSnapshot(snapshot: FolderViewsSnapshot): Map<string, FolderView> {
	return new Map(snapshot.folders.map((entry) => [entry.key, entry.view]));
}

/**
 * Subscribes to changes first and reads the snapshot second, so no change falls between the two.
 * A change is applied only when its revision is the next one: an older one was already in the
 * snapshot, and a gap means one was missed, so the snapshot is read again. A write is answered
 * with a revision and also arrives as an event; the second is skipped.
 */
export function createFolderViewsStore(client: FolderViewsClient): FolderViewsHandle {
	const store = createStore<FolderViewsState>()(() => ({
		folders: new Map(),
		revision: 0,
		ready: false,
		writing: 0,
	}));
	let disposed = false;
	// Events that arrive before the snapshot, applied in order once it is in.
	let waiting: FolderViewsChanged[] | null = [];

	const applySnapshot = (snapshot: FolderViewsSnapshot) => {
		const state = store.getState();
		if (state.ready && snapshot.revision <= state.revision) return;
		store.setState({
			folders: fromSnapshot(snapshot),
			revision: snapshot.revision,
			ready: true,
		});
	};

	const refetch = () =>
		client.snapshot().then(
			(snapshot) => {
				if (!disposed) applySnapshot(snapshot);
			},
			(error: unknown) => console.warn('could not read the folder views', error),
		);

	const applyChange = (changed: FolderViewsChanged) => {
		const state = store.getState();
		if (changed.revision <= state.revision) return;
		if (changed.revision > state.revision + 1) {
			void refetch();
			return;
		}
		const folders = new Map(state.folders);
		for (const change of changed.changes) {
			if (change.view === null) folders.delete(change.key);
			else folders.set(change.key, change.view);
		}
		store.setState({ folders, revision: changed.revision });
	};

	const unsubscribe = client.onChanged((changed) => {
		if (disposed) return;
		if (waiting) waiting.push(changed);
		else applyChange(changed);
	});
	const ready = client.snapshot().then(
		(snapshot) => {
			if (disposed) return;
			applySnapshot(snapshot);
			const queued = waiting ?? [];
			waiting = null;
			for (const changed of queued) applyChange(changed);
		},
		(error: unknown) => {
			console.warn('could not read the folder views; no folder remembers one', error);
			waiting = null;
		},
	);

	const writing = async (write: () => Promise<number>) => {
		store.setState((state) => ({ writing: state.writing + 1 }));
		try {
			await write();
		} finally {
			store.setState((state) => ({ writing: state.writing - 1 }));
		}
	};

	return {
		store,
		ready,
		remember: (key, choice) =>
			writing(() => client.remember(key, { mode: choice.mode ?? null, sort: choice.sort ?? null })),
		reset: (key) => writing(() => client.reset(key)),
		dispose() {
			disposed = true;
			unsubscribe();
		},
	};
}
