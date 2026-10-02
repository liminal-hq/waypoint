// What the Settings pages share: the combined settings, saving one change at a time, and the error under each row
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	createContext,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useState,
	type ReactNode,
} from 'react';
import { useStore } from 'zustand';
import type { PluginStatus as WindowEffectsStatus } from '@liminal-hq/plugin-window-effects';
import type { PluginStatus } from '@liminal-hq/plugin-thumbnails';
import { t, tf } from '../i18n/messages';
import type { OpsSettings } from '../services/opsClient';
import { isSettingsError, type Settings } from '../services/settingsClient';
import type { SettingsHandle } from './settingsStore';

/** The rows that can show an error, by the setting each one edits. */
export type RowKey =
	| 'startup'
	| 'defaultView'
	| 'clickMode'
	| 'showHidden'
	| 'appNameInTitle'
	| 'menuBar'
	| 'confirmTrash'
	| 'verify'
	| 'algorithm'
	| 'concurrency'
	| 'undoDepth'
	| 'trashExpiry'
	| 'trashDays'
	| 'dropRule'
	| 'springLoad'
	| 'shelfPersist'
	| 'mode'
	| 'themeSource'
	| 'accent'
	| 'accentColour'
	| 'density'
	| 'iconStyle'
	| 'highContrast'
	| 'textSize'
	| 'strongFocus'
	| 'reducedMotion'
	| 'reducedTransparency'
	| 'touchMode'
	| 'thumbnails'
	| 'thumbnailMax'
	| 'measureHomeOnOpen'
	| 'transparency'
	| 'opacity'
	| 'blur'
	| 'regionTitleBar'
	| 'regionSidebar'
	| 'regionContent'
	| 'menus'
	| 'menuOpacity'
	| 'solidUnfocused';

/** The operations plugin's settings commands, which the Settings window edits the operations settings through. */
export interface OpsSettingsApi {
	getSettings(): Promise<OpsSettings>;
	setSettings(settings: OpsSettings): Promise<OpsSettings>;
}

/** Whether the system can drag files out to other applications, and why not when it cannot. */
export interface DndAvailability {
	outbound: { available: boolean; reason: string | null };
}

export interface SettingsEditor {
	/** Both documents have been read, so the rows show Rust's values and not placeholders. */
	ready: boolean;
	settings: Settings;
	/** `null` until read, and for good when the operations plugin could not be read. */
	ops: OpsSettings | null;
	/** Why the operations settings cannot be changed, when they could not be read. */
	opsUnreadable: string | null;
	dnd: DndAvailability | null;
	/** What the thumbnails plugin can do here; `null` until read, and when it could not be. */
	thumbnails: PluginStatus | null;
	/** What the window effects plugin can do here; `null` until read, and when it could not be. */
	windowEffects: WindowEffectsStatus | null;
	errors: Partial<Record<RowKey, string>>;
	/** Asks Rust for the settings `change` makes of the ones in force. The row shows the answer, never the request. */
	changeSettings(key: RowKey, change: (settings: Settings) => Settings): void;
	changeOps(key: RowKey, change: (ops: OpsSettings) => OpsSettings): void;
}

const SettingsEditorContext = createContext<SettingsEditor | null>(null);

export function useSettingsEditor(): SettingsEditor {
	const editor = useContext(SettingsEditorContext);
	if (!editor) throw new Error('useSettingsEditor must be used inside a SettingsEditorProvider');
	return editor;
}

/** The sentence under a row for a refused or failed change. */
export function errorMessage(error: unknown): string {
	if (isSettingsError(error) && error.min !== undefined && error.max !== undefined) {
		return tf('settings.error.range', { min: error.min, max: error.max });
	}
	// A refused save already says so in Rust's own words ("could not save the settings: …").
	if (isSettingsError(error) && error.kind === 'storage') {
		return error.message.charAt(0).toUpperCase() + error.message.slice(1);
	}
	const reason =
		isSettingsError(error) || error instanceof Error ? error.message : t('settings.error.generic');
	return tf('settings.error.saveFailed', { reason });
}

interface SettingsEditorProviderProps {
	handle: SettingsHandle;
	ops: OpsSettingsApi;
	/** Reads what drag and drop can do on this system; a failure leaves the page without the note. */
	dndStatus?: () => Promise<DndAvailability>;
	/** Reads what thumbnails can do on this system, for the Previews & thumbnails page; a failure leaves the page without the note. */
	thumbnailsStatus?: () => Promise<PluginStatus>;
	/** Reads what window effects can do on this system, for the Transparency page; a failure leaves the page saying it could not tell. */
	windowEffectsStatus?: () => Promise<WindowEffectsStatus>;
	children: ReactNode;
}

/**
 * Follows both documents and saves one change at a time, so a second change is made from the
 * answer to the first and never from a value that is already out of date. A refused change is
 * shown under its row and the row keeps the value that is in force.
 */
export function SettingsEditorProvider({
	handle,
	ops: opsApi,
	dndStatus,
	thumbnailsStatus,
	windowEffectsStatus,
	children,
}: SettingsEditorProviderProps) {
	const { settings, ready: settingsReady } = useStore(handle.store, (state) => state);
	const [ops, setOps] = useState<OpsSettings | null>(null);
	const [opsReady, setOpsReady] = useState(false);
	const [opsUnreadable, setOpsUnreadable] = useState<string | null>(null);
	const [dnd, setDnd] = useState<DndAvailability | null>(null);
	const [thumbnails, setThumbnails] = useState<PluginStatus | null>(null);
	const [windowEffects, setWindowEffects] = useState<WindowEffectsStatus | null>(null);
	const [errors, setErrors] = useState<Partial<Record<RowKey, string>>>({});
	const opsNow = useRef<OpsSettings | null>(null);
	const queue = useRef<Promise<void>>(Promise.resolve());

	useEffect(() => {
		let active = true;
		opsApi.getSettings().then(
			(value) => {
				if (!active) return;
				opsNow.current = value;
				setOps(value);
				setOpsReady(true);
			},
			(error: unknown) => {
				if (!active) return;
				console.warn('could not read the operations settings', error);
				setOpsUnreadable(isSettingsError(error) ? error.message : t('settings.error.generic'));
				setOpsReady(true);
			},
		);
		return () => {
			active = false;
		};
	}, [opsApi]);

	useEffect(() => {
		if (!dndStatus) return;
		let active = true;
		dndStatus().then(
			(value) => {
				if (active) setDnd(value);
			},
			(error: unknown) => console.warn('could not read the drag and drop status', error),
		);
		return () => {
			active = false;
		};
	}, [dndStatus]);

	useEffect(() => {
		if (!thumbnailsStatus) return;
		let active = true;
		thumbnailsStatus().then(
			(value) => {
				if (active) setThumbnails(value);
			},
			(error: unknown) => console.warn('could not read the thumbnails status', error),
		);
		return () => {
			active = false;
		};
	}, [thumbnailsStatus]);

	useEffect(() => {
		if (!windowEffectsStatus) return;
		let active = true;
		windowEffectsStatus().then(
			(value) => {
				if (active) setWindowEffects(value);
			},
			(error: unknown) => console.warn('could not read the window effects status', error),
		);
		return () => {
			active = false;
		};
	}, [windowEffectsStatus]);

	const run = useCallback((key: RowKey, work: () => Promise<void>) => {
		setErrors((current) => {
			if (!(key in current)) return current;
			const { [key]: _cleared, ...rest } = current;
			return rest;
		});
		queue.current = queue.current.then(async () => {
			try {
				await work();
			} catch (error) {
				setErrors((current) => ({ ...current, [key]: errorMessage(error) }));
			}
		});
	}, []);

	const changeSettings = useCallback<SettingsEditor['changeSettings']>(
		(key, change) =>
			run(key, async () => {
				await handle.save(change(handle.store.getState().settings));
			}),
		[handle, run],
	);

	const changeOps = useCallback<SettingsEditor['changeOps']>(
		(key, change) =>
			run(key, async () => {
				const current = opsNow.current;
				if (!current) return;
				const saved = await opsApi.setSettings(change(current));
				opsNow.current = saved;
				setOps(saved);
			}),
		[opsApi, run],
	);

	const value = useMemo<SettingsEditor>(
		() => ({
			ready: settingsReady && opsReady,
			settings,
			ops,
			opsUnreadable,
			dnd,
			thumbnails,
			windowEffects,
			errors,
			changeSettings,
			changeOps,
		}),
		[
			settingsReady,
			opsReady,
			settings,
			ops,
			opsUnreadable,
			dnd,
			thumbnails,
			windowEffects,
			errors,
			changeSettings,
			changeOps,
		],
	);
	return <SettingsEditorContext.Provider value={value}>{children}</SettingsEditorContext.Provider>;
}
