// An in-memory SettingsClient with Rust's contract: validation, a rising revision and an event to every listener
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	DEFAULT_SETTINGS,
	MENU_OPACITY_MIN,
	OPACITY_MAX,
	OPACITY_MIN,
	PREVIEW_MAX_MB_MAX,
	PREVIEW_MAX_MB_MIN,
	SPRING_LOAD_MAX_MS,
	SPRING_LOAD_MIN_MS,
	SUPPORTED_LANGUAGES,
	TEXT_SIZES,
	type Settings,
	type SettingsClient,
	type SettingsCommandError,
	type SettingsSnapshot,
} from './settingsClient';

export interface FakeSettings extends SettingsClient {
	/** Every `setUi` the page sent, in order. */
	readonly uiCalls: Array<Partial<Settings['ui']>>;
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

/** The refusal Rust gives for a value that is not one of the allowed ones. */
export function invalidError(field: string, reason: string): SettingsCommandError {
	return { kind: 'invalid', message: `${field} is not valid: ${reason}`, field };
}

/** Rust's checks for the milestone 5 sections, so a page tested against the fake meets the same refusals. */
function refusal(settings: Settings): SettingsCommandError | null {
	const ranges: [string, number, number, number][] = [
		['transparency.opacity', settings.transparency.opacity, OPACITY_MIN, OPACITY_MAX],
		['transparency.menuOpacity', settings.transparency.menuOpacity, MENU_OPACITY_MIN, OPACITY_MAX],
		['previews.maxFileMb', settings.previews.maxFileMb, PREVIEW_MAX_MB_MIN, PREVIEW_MAX_MB_MAX],
	];
	for (const [field, value, min, max] of ranges) {
		if (value < min || value > max) return rangeError(field, min, max);
	}
	if (!(TEXT_SIZES as readonly number[]).includes(settings.accessibility.textSize)) {
		return invalidError('accessibility.textSize', 'use 100, 115 or 130');
	}
	const accent = settings.appearance.accent;
	if (accent.kind === 'custom' && !/^#[0-9a-fA-F]{6}$/.test(accent.hex)) {
		return invalidError('appearance.accent', 'write it as #rrggbb');
	}
	const language = settings.locale.language;
	if (language !== 'system' && !(SUPPORTED_LANGUAGES as readonly string[]).includes(language)) {
		return invalidError('locale.language', 'not a language Waypoint has');
	}
	const shortcut = settings.integrations.globalShortcut;
	if (shortcut !== null && (shortcut.trim() === '' || shortcut.length > 64)) {
		return invalidError('integrations.globalShortcut', 'write it like Ctrl+Alt+W');
	}
	return null;
}

export function createFakeSettingsClient(initial: Settings = DEFAULT_SETTINGS): FakeSettings {
	let state: SettingsSnapshot = { revision: 0, settings: initial };
	const listeners = new Set<(snapshot: SettingsSnapshot) => void>();
	const calls: Settings[] = [];
	const uiCalls: Array<Partial<Settings['ui']>> = [];
	let failure: SettingsCommandError | null = null;
	let hold: Promise<void> | null = null;

	const announce = (snapshot: SettingsSnapshot) => {
		for (const listener of [...listeners]) listener(snapshot);
	};

	return {
		calls,
		uiCalls,
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
			const refused = refusal(settings);
			if (refused) throw refused;
			if (JSON.stringify(settings) === JSON.stringify(state.settings)) return state;
			state = { revision: state.revision + 1, settings };
			announce(state);
			return state;
		},
		async setUi(change) {
			uiCalls.push(change);
			if (failure) {
				const error = failure;
				failure = null;
				throw error;
			}
			// Rust merges onto what is in force, not onto what the caller last saw.
			const merged = { ...state.settings, ui: { ...state.settings.ui, ...change } };
			if (JSON.stringify(merged) === JSON.stringify(state.settings)) return state;
			state = { revision: state.revision + 1, settings: merged };
			announce(state);
			return state;
		},
		onChanged(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}
