// Holds a folder's list column widths: the ones Rust remembers for it, and the one being chosen by a drag or a run of key presses
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useStore } from 'zustand';
import type { ListColumnWidths } from '../services/folderViewsClient';
import { isOverviewLocation } from '../overview/overviewLocation';
import { useSettings, useSettingsReady } from '../settings/SettingsContext';
import { IDLE_FOLDER_VIEWS } from './folderViewStore';
import { useFolderViews } from './FolderViewsContext';
import {
	noWidths,
	resolveWidths,
	withWidth,
	type ColumnWidths,
	type ResizableColumn,
} from './columnWidths';

/** How long the keys rest before the width they reached is kept, so a held arrow key saves once. */
const KEY_SETTLE_MS = 400;

/** A width being chosen: a number, `null` for the column's own width, and nothing once it is kept or dropped. */
type Draft = Partial<Record<ResizableColumn, number | null>>;

const selectRemembering = (settings: { general: { rememberFolderViews: boolean } }) =>
	settings.general.rememberFolderViews;

/**
 * The key a folder's widths are remembered under: its location's `uri`, as for its view and sort.
 * Unlike them, the Trash remembers its widths too (its columns are its own), so only Overview,
 * which is not a listing, has none.
 */
export function columnWidthsKey(location: Location): string | null {
	return isOverviewLocation(location) ? null : location.uri;
}

export interface ColumnWidthControls {
	/** The widths in force, with any being chosen shown. */
	widths: ColumnWidths;
	/** Whether any column is not at its own width, so there is something to reset. */
	resized: boolean;
	/** Shows a width without keeping it; `undefined` drops it. */
	preview(column: ResizableColumn, width: number | null | undefined): void;
	/** Keeps a width for this folder, or `null` for the column's own. */
	commit(column: ResizableColumn, width: number | null): void;
	/** Keeps a width once the keys have rested. */
	commitSoon(column: ResizableColumn, width: number | null): void;
	/** Keeps a width still waiting on the keys. */
	flush(): void;
	/** Puts every column of this folder back to its own width. */
	resetAll(): void;
}

/**
 * The widths are the folder's own, like its view and sort: Rust keeps them with the folder's
 * remembered view, so every window showing the folder hears a change and a new tab on it starts
 * with them, and another folder keeps the widths it has. A drag only shows its width until it is
 * released. Where folders do not remember (the setting is off, or no service is reachable) a
 * width stays shown in this listing only.
 */
export function useColumnWidths(location: Location): ColumnWidthControls {
	const key = columnWidthsKey(location);
	const handle = useFolderViews();
	const wanted = useSettings(selectRemembering);
	const settingsReady = useSettingsReady();
	const remembering = wanted && settingsReady && handle !== null;
	const stored = useStore(handle?.store ?? IDLE_FOLDER_VIEWS, (state) =>
		remembering && key !== null ? (state.folders.get(key)?.columnWidths ?? null) : null,
	);
	const [draft, setDraft] = useState<Draft>({});
	const pending = useRef<{ column: ResizableColumn; width: number | null } | null>(null);
	const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

	const widths = useMemo(() => {
		const shown = resolveWidths(stored);
		for (const [column, width] of Object.entries(draft) as [ResizableColumn, number | null][]) {
			if (width === null) delete shown[column];
			else shown[column] = width;
		}
		return shown;
	}, [stored, draft]);

	const preview = useCallback((column: ResizableColumn, width: number | null | undefined) => {
		setDraft((now) => {
			const next = { ...now };
			if (width === undefined) delete next[column];
			else next[column] = width;
			return next;
		});
	}, []);

	// A kept width is shown from the draft until the folder's memory says the same, so it does not
	// jump back for the moment between Rust's answer and its event.
	useEffect(() => {
		setDraft((now) => {
			const kept = Object.entries(now).filter(
				([column, width]) => (stored?.[column as ResizableColumn] ?? null) !== width,
			);
			return kept.length === Object.keys(now).length ? now : Object.fromEntries(kept);
		});
	}, [stored, draft]);

	const save = useCallback(
		(change: (current: ListColumnWidths) => ListColumnWidths, failed: () => void) => {
			if (!remembering || !handle || key === null) return;
			const current = handle.store.getState().folders.get(key)?.columnWidths ?? noWidths();
			handle.remember(key, { columnWidths: change(current) }).catch((error: unknown) => {
				console.warn('could not remember the column widths for the folder', error);
				failed();
			});
		},
		[remembering, handle, key],
	);

	const commit = useCallback(
		(column: ResizableColumn, width: number | null) => {
			if (timer.current !== null) clearTimeout(timer.current);
			timer.current = null;
			pending.current = null;
			preview(column, width);
			save(
				(current) => withWidth(current, column, width),
				() => preview(column, undefined),
			);
		},
		[preview, save],
	);

	const flush = useCallback(() => {
		const waiting = pending.current;
		if (!waiting) return;
		commit(waiting.column, waiting.width);
	}, [commit]);

	const commitSoon = useCallback(
		(column: ResizableColumn, width: number | null) => {
			if (pending.current && pending.current.column !== column) flush();
			if (timer.current !== null) clearTimeout(timer.current);
			pending.current = { column, width };
			preview(column, width);
			timer.current = setTimeout(flush, KEY_SETTLE_MS);
		},
		[flush, preview],
	);

	const resetAll = useCallback(() => {
		if (timer.current !== null) clearTimeout(timer.current);
		timer.current = null;
		pending.current = null;
		setDraft(noWidths());
		save(noWidths, () => setDraft({}));
	}, [save]);

	// Leaving the folder keeps what the keys last asked for.
	useEffect(() => flush, [flush]);

	return {
		widths,
		resized: Object.keys(widths).length > 0,
		preview,
		commit,
		commitSoon,
		flush,
		resetAll,
	};
}
