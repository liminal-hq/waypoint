// An in-memory FolderViewsClient with Rust's contract: merged patches, a bound, a rising revision and a granular event
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type {
	FolderView,
	FolderViewPatch,
	FolderViewsChanged,
	FolderViewsClient,
	FolderViewsSnapshot,
} from './folderViewsClient';

/** The bound Rust keeps (`MAX_FOLDERS` in `waypoint-settings`). */
export const MAX_FOLDERS = 1000;

export interface FakeFolderViews extends FolderViewsClient {
	/** Every `remember` the window sent, in order. */
	readonly remembered: Array<{ key: string; patch: FolderViewPatch }>;
	/** Every `reset` the window sent, in order. */
	readonly resets: string[];
	/** What a folder remembers now. */
	view(key: string): FolderView | undefined;
	/** Changes a folder as another window would: stored and announced to every listener. */
	change(key: string, patch: FolderViewChoice): void;
	/** Sends an event as the plugin would, without changing what is stored (a late, repeated or missed one). */
	emit(changed: FolderViewsChanged): void;
	/** Makes `snapshot` slow: it settles when `release` is called. */
	holdSnapshot(): { release(): void };
	/** Makes writes slow: they settle when `release` is called, after the event they cause. */
	holdWrites(): { release(): void };
	/** Rejects the next write, as a failed save would. */
	failNext(): void;
}

/** What a test says a folder remembers: the fields it leaves out were never chosen. */
export type FolderViewSeed = Partial<FolderView>;

/** A patch with only the fields a test names; the rest keep what the folder has. */
export type FolderViewChoice = Partial<FolderViewPatch>;

/** A patch that chooses nothing: what a test spreads its one choice onto. */
export const NOTHING_CHOSEN: FolderViewPatch = {
	mode: null,
	sort: null,
	showHidden: null,
	iconSize: null,
};

const complete = (view: FolderViewSeed): FolderView => ({
	mode: view.mode ?? null,
	sort: view.sort ?? null,
	showHidden: view.showHidden ?? null,
	iconSize: view.iconSize ?? null,
});

export function createFakeFolderViewsClient(
	initial: Record<string, FolderViewSeed> = {},
): FakeFolderViews {
	// Oldest write first, like the store in Rust.
	const entries = new Map<string, FolderView>(
		Object.entries(initial).map(([key, view]) => [key, complete(view)]),
	);
	let revision = 0;
	const listeners = new Set<(changed: FolderViewsChanged) => void>();
	const remembered: Array<{ key: string; patch: FolderViewPatch }> = [];
	const resets: string[] = [];
	let snapshotHold: Promise<void> | null = null;
	let writeHold: Promise<void> | null = null;
	let failure = false;

	const announce = (changed: FolderViewsChanged) => {
		for (const listener of [...listeners]) listener(changed);
	};
	const snapshotOf = (): FolderViewsSnapshot => ({
		revision,
		folders: [...entries].map(([key, view]) => ({ key, view })),
	});
	const write = (key: string, patch: FolderViewChoice) => {
		const before = entries.get(key);
		const patched = complete(patch);
		const view: FolderView = {
			mode: patched.mode ?? before?.mode ?? null,
			sort: patched.sort ?? before?.sort ?? null,
			showHidden: patched.showHidden ?? before?.showHidden ?? null,
			iconSize: patched.iconSize ?? before?.iconSize ?? null,
		};
		if (Object.values(view).every((value) => value === null)) return;
		if (before && JSON.stringify(before) === JSON.stringify(view)) return;
		entries.delete(key);
		entries.set(key, view);
		const changes = [{ key, view }] as FolderViewsChanged['changes'];
		while (entries.size > MAX_FOLDERS) {
			const oldest = entries.keys().next().value as string;
			entries.delete(oldest);
			changes.push({ key: oldest, view: null });
		}
		revision += 1;
		announce({ revision, changes });
	};

	return {
		remembered,
		resets,
		view: (key) => entries.get(key),
		change: (key, patch) => write(key, patch),
		emit: announce,
		holdSnapshot() {
			let release = () => {};
			snapshotHold = new Promise<void>((resolve) => {
				release = resolve;
			});
			return { release };
		},
		holdWrites() {
			let release = () => {};
			writeHold = new Promise<void>((resolve) => {
				release = resolve;
			});
			return { release };
		},
		failNext() {
			failure = true;
		},
		async snapshot() {
			if (snapshotHold) await snapshotHold;
			return snapshotOf();
		},
		async remember(key, patch) {
			remembered.push({ key, patch });
			if (failure) {
				failure = false;
				throw { kind: 'storage', message: 'could not save the folder views' };
			}
			write(key, patch);
			if (writeHold) await writeHold;
			return revision;
		},
		async reset(key) {
			resets.push(key);
			if (failure) {
				failure = false;
				throw { kind: 'storage', message: 'could not save the folder views' };
			}
			if (entries.delete(key)) {
				revision += 1;
				announce({ revision, changes: [{ key, view: null }] });
			}
			if (writeHold) await writeHold;
			return revision;
		},
		onChanged(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}
