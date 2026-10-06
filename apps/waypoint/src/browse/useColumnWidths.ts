// Holds the list's column widths: the ones Rust remembers, and the one being chosen by a drag or a run of key presses
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { ListColumnWidths, Settings } from '../services/settingsClient';
import { useSettings, useSettingsHandle } from '../settings/SettingsContext';
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

const selectWidths = (settings: Settings): ListColumnWidths => settings.ui.columnWidths;

export interface ColumnWidthControls {
	/** The widths in force, with any being chosen shown. */
	widths: ColumnWidths;
	/** Whether any column is not at its own width, so there is something to reset. */
	resized: boolean;
	/** Shows a width without keeping it; `undefined` drops it. */
	preview(column: ResizableColumn, width: number | null | undefined): void;
	/** Keeps a width for every folder, or `null` for the column's own. */
	commit(column: ResizableColumn, width: number | null): void;
	/** Keeps a width once the keys have rested. */
	commitSoon(column: ResizableColumn, width: number | null): void;
	/** Keeps a width still waiting on the keys. */
	flush(): void;
	/** Puts every column back to its own width. */
	resetAll(): void;
}

/**
 * The widths are one set for every folder and every window, so Rust keeps them with the `ui`
 * settings and every window hears the change. A drag only shows its width until it is released;
 * without a settings store (a window that cannot reach Rust) the width simply stays shown.
 */
export function useColumnWidths(): ColumnWidthControls {
	const stored = useSettings(selectWidths);
	const handle = useSettingsHandle();
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

	const save = useCallback(
		(change: (current: ListColumnWidths) => ListColumnWidths, settled: () => void) => {
			if (!handle) return;
			const current = handle.store.getState().settings.ui.columnWidths;
			handle
				.saveUi({ columnWidths: change(current) })
				.catch((error: unknown) => {
					console.warn('could not save the column widths', error);
				})
				.finally(settled);
		},
		[handle],
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
		setDraft({});
		save(noWidths, () => {});
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
