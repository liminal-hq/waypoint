// The application settings as a window's copy of Rust's: one snapshot kept current by revision-gated events
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createStore, type StoreApi } from 'zustand/vanilla';
import {
	DEFAULT_SETTINGS,
	type Settings,
	type SettingsClient,
	type SettingsSnapshot,
	type UiSettings,
} from '../services/settingsClient';

export interface SettingsState {
	settings: Settings;
	/** The revision `settings` is at; 0 until the first snapshot (the defaults) and for what Rust loaded at start-up. */
	revision: number;
	/** Whether the first snapshot has arrived, so what is shown is Rust's and not the defaults. */
	ready: boolean;
}

export interface SettingsHandle {
	store: StoreApi<SettingsState>;
	/** Settles once the first snapshot is in (or could not be read, and the defaults stay). */
	ready: Promise<void>;
	/**
	 * Asks Rust for `next`. Resolves to the snapshot in force; rejects with Rust's refusal and
	 * leaves the store as it was. The store changes only by what Rust says is in force.
	 */
	save(next: Settings): Promise<SettingsSnapshot>;
	/** Asks Rust to change only the named `ui` settings; what the main windows use, since they never send the whole document. */
	saveUi(change: Partial<UiSettings>): Promise<SettingsSnapshot>;
	/** Stops following. The store keeps what it holds. */
	dispose(): void;
}

/**
 * Subscribes to changes first and reads the snapshot second, so no change falls between the two. A
 * snapshot or event is applied only when its revision is newer than what the store holds, so a
 * late or repeated one never turns the settings back (the answer to this window's own `save` and
 * the event that follows it are the same revision, and the second is skipped).
 */
export function createSettingsStore(client: SettingsClient): SettingsHandle {
	const store = createStore<SettingsState>()(() => ({
		settings: DEFAULT_SETTINGS,
		revision: 0,
		ready: false,
	}));
	let disposed = false;

	const apply = (snapshot: SettingsSnapshot) => {
		if (disposed) return;
		const state = store.getState();
		// The first snapshot is accepted at any revision: 0 is a real revision (what was loaded).
		if (state.ready && snapshot.revision <= state.revision) return;
		store.setState({ settings: snapshot.settings, revision: snapshot.revision, ready: true });
	};

	const unsubscribe = client.onChanged(apply);
	const ready = client.snapshot().then(
		(snapshot) => apply(snapshot),
		(error: unknown) => console.warn('could not read the settings; using the defaults', error),
	);

	return {
		store,
		ready,
		async save(next) {
			const snapshot = await client.set(next);
			apply(snapshot);
			return snapshot;
		},
		async saveUi(change) {
			const snapshot = await client.setUi(change);
			apply(snapshot);
			return snapshot;
		},
		dispose() {
			disposed = true;
			unsubscribe();
		},
	};
}
