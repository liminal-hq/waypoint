// An in-memory SettingsClient with Rust's contract: validation, a rising revision and an event to every listener
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	DEFAULT_SETTINGS,
	SPRING_LOAD_MAX_MS,
	SPRING_LOAD_MIN_MS,
	type Settings,
	type SettingsClient,
	type SettingsCommandError,
	type SettingsSnapshot,
} from './settingsClient';

export interface FakeSettings extends SettingsClient {
	/** Every `set` the page sent, in order, whether or not it was accepted. */
	readonly calls: Settings[];
	/** The settings in force. */
	current(): SettingsSnapshot;
	/** Refuses the next `set` with `error`, as a failed save would. */
	failNext(error: SettingsCommandError): void;
	/** Changes the settings as another window would: stored and announced to every listener. */
	change(settings: Settings): SettingsSnapshot;
	/** Sends an event as the plugin would, without changing what is in force (a late or repeated one). */
	emit(snapshot: SettingsSnapshot): void;
	/** Makes `snapshot` slow: it settles when `release` is called. */
	holdSnapshot(): { release(): void };
}

/** The refusal Rust gives for a value out of range. */
export function rangeError(field: string, min: number, max: number): SettingsCommandError {
	return {
		kind: 'invalid',
		message: `${field} must be between ${min} and ${max}`,
		field,
		min,
		max,
	};
}

export function createFakeSettingsClient(initial: Settings = DEFAULT_SETTINGS): FakeSettings {
	let state: SettingsSnapshot = { revision: 0, settings: initial };
	const listeners = new Set<(snapshot: SettingsSnapshot) => void>();
	const calls: Settings[] = [];
	let failure: SettingsCommandError | null = null;
	let hold: Promise<void> | null = null;

	const announce = (snapshot: SettingsSnapshot) => {
		for (const listener of [...listeners]) listener(snapshot);
	};

	return {
		calls,
		current: () => state,
		failNext(error) {
			failure = error;
		},
		change(settings) {
			state = { revision: state.revision + 1, settings };
			announce(state);
			return state;
		},
		emit: announce,
		holdSnapshot() {
			let release = () => {};
			hold = new Promise<void>((resolve) => {
				release = resolve;
			});
			return { release };
		},
		async snapshot() {
			if (hold) await hold;
			return state;
		},
		async set(settings) {
			calls.push(settings);
			if (failure) {
				const error = failure;
				failure = null;
				throw error;
			}
			const ms = settings.dnd.springLoadMs;
			if (ms < SPRING_LOAD_MIN_MS || ms > SPRING_LOAD_MAX_MS) {
				throw rangeError('dnd.springLoadMs', SPRING_LOAD_MIN_MS, SPRING_LOAD_MAX_MS);
			}
			if (JSON.stringify(settings) === JSON.stringify(state.settings)) return state;
			state = { revision: state.revision + 1, settings };
			announce(state);
			return state;
		},
		onChanged(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}
