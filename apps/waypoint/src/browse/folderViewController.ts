// Decides what each folder shows: its own remembered view and sort, or the window's, and writes the choices made in it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A folder remembers only what was chosen in it (SPEC 5.3b): its view, sort and grouping, hidden
// files and icon size. Everything else is the window's: its starting sort and grouping, and the
// view, hidden-files choice and icon size it started with. The window's choices change only while
// remembering is off, so turning it off, and on again, always has the same meaning: a folder with
// a remembered choice shows it, and every other folder shows the window's.
//
// The sort is each listing's own (Rust sorts it), so it is applied when a listing opens and,
// afterwards, through `applySorts`. The view mode, hidden files and icon size are the window's one
// value each, so they follow the folder in the active tab.

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import { isOverviewLocation } from '../overview/overviewLocation';
import { isTrashLocation } from '../trash/trashLocation';
import type { FolderViewsHandle } from './folderViewStore';
import type { ListingManager } from './listingManager';
import { clampGridSize, sameSort, type ViewMode, type ViewStore } from './viewStore';

/**
 * The key a folder is remembered under: its location's `uri`, which Rust makes canonical. `null`
 * for what is not a folder that can remember (no location, Overview, the Trash, whose columns and
 * view are its own).
 */
export function folderViewKey(location: Location | null | undefined): string | null {
	if (!location || isOverviewLocation(location) || isTrashLocation(location)) return null;
	return location.uri;
}

/** What the controller reads that changes between renders. */
export interface FolderViewInput {
	/** The remembered views, or `null` where there is no service. */
	handle: FolderViewsHandle | null;
	/** The General setting (and whether the settings have been read: until then nothing is written). */
	enabled: boolean;
	/** The folder the active tab shows, as `folderViewKey` gives it. */
	activeKey: string | null;
}

/** How long a run of icon size changes (a slider drag) waits before the folder keeps the last one. */
export const ICON_SIZE_WRITE_DELAY_MS = 400;

/** The window values that follow the folder in the active tab (the sort is each listing's own). */
interface WindowView {
	mode: ViewMode;
	showHidden: boolean;
	gridSize: number;
}

export class FolderViewController {
	private input: FolderViewInput = { handle: null, enabled: false, activeKey: null };
	/** What folders without a choice of their own show: the window's, changed only while remembering is off. */
	private baseline: WindowView;
	/** Set while the controller itself changes the window's values, so the change is not mistaken for a choice. */
	private applying = false;
	/** An icon size change waiting for the slider to settle. */
	private pendingSize: { key: string; size: number; timer: ReturnType<typeof setTimeout> } | null =
		null;

	constructor(private viewStore: ViewStore) {
		this.baseline = windowView(viewStore.getState());
	}

	configure(input: FolderViewInput): void {
		// A size chosen in the folder being left is kept by that folder, not the next one.
		if (this.pendingSize && this.pendingSize.key !== input.activeKey) this.flushSize();
		this.input = input;
	}

	/** The remembering that is in force: on, and a service to remember with. */
	private get active(): FolderViewsHandle | null {
		return this.input.enabled ? this.input.handle : null;
	}

	private remembered(key: string) {
		return this.active?.store.getState().folders.get(key);
	}

	/** The sort a listing of `location` should have: the folder's own, else the window's. */
	sortFor = (location: Location): SortSpec => {
		const key = folderViewKey(location);
		return (key && this.remembered(key)?.sort) || this.viewStore.getState().sort;
	};

	/**
	 * A folder listing's sort changed. With remembering on the folder keeps it, unless it is what
	 * the folder already shows (the controller's own `applySorts`, a reset). With it off the window
	 * keeps it, as it always did.
	 */
	onSort = (sort: SortSpec, location: Location): void => {
		const handle = this.active;
		const key = folderViewKey(location);
		if (!handle) return this.viewStore.getState().setSort(sort);
		if (!key) return;
		if (sameSort(sort, this.sortFor(location))) return;
		handle.remember(key, { sort }).catch(warn('the sort'));
	};

	/**
	 * Follows the window's view mode, hidden-files choice and icon size: a change made by the person
	 * is remembered by the folder, or by the window while remembering is off. Returns the function
	 * that stops following (a size still waiting is kept first).
	 */
	followMode(): () => void {
		let last = windowView(this.viewStore.getState());
		const stop = this.viewStore.subscribe((state) => {
			const now = windowView(state);
			const before = last;
			last = now;
			if (this.applying) return;
			const handle = this.active;
			const key = this.input.activeKey;
			if (!handle) {
				this.baseline = now;
				return;
			}
			if (!key) return;
			if (now.mode !== before.mode)
				handle.remember(key, { mode: now.mode }).catch(warn('the view'));
			if (now.showHidden !== before.showHidden) {
				handle.remember(key, { showHidden: now.showHidden }).catch(warn('hidden files'));
			}
			if (now.gridSize !== before.gridSize) this.scheduleSize(key, now.gridSize);
		});
		return () => {
			stop();
			this.flushSize();
		};
	}

	private scheduleSize(key: string, size: number): void {
		if (this.pendingSize) clearTimeout(this.pendingSize.timer);
		const timer = setTimeout(() => this.flushSize(), ICON_SIZE_WRITE_DELAY_MS);
		this.pendingSize = { key, size, timer };
	}

	private flushSize(): void {
		const pending = this.pendingSize;
		if (!pending) return;
		clearTimeout(pending.timer);
		this.pendingSize = null;
		this.active?.remember(pending.key, { iconSize: pending.size }).catch(warn('the icon size'));
	}

	/** Brings the window's view values and every open listing's sort in line with what the folders remember. */
	reconcile(manager: ListingManager): void {
		const handle = this.active;
		// Rust has not answered this window's own writes: what is on screen is the newer choice.
		if (handle && handle.store.getState().writing > 0) return;
		this.reconcileView();
		manager.applySorts(this.sortFor);
	}

	private reconcileView(): void {
		const key = this.input.activeKey;
		let wanted: WindowView;
		if (!this.active) wanted = this.baseline;
		// The Trash and Overview keep the view they have.
		else if (key) {
			const own = this.remembered(key);
			wanted = {
				mode: own?.mode ?? this.baseline.mode,
				showHidden: own?.showHidden ?? this.baseline.showHidden,
				gridSize: own?.iconSize ? clampGridSize(own.iconSize) : this.baseline.gridSize,
			};
		} else return;
		const now = windowView(this.viewStore.getState());
		if (sameWindowView(now, wanted)) return;
		this.applying = true;
		try {
			const state = this.viewStore.getState();
			if (now.mode !== wanted.mode) state.setMode(wanted.mode);
			if (now.showHidden !== wanted.showHidden) state.toggleHidden();
			if (now.gridSize !== wanted.gridSize) state.setGridSize(wanted.gridSize);
		} finally {
			this.applying = false;
		}
	}

	/** Whether the folder at `key` remembers something to reset. */
	canReset(key: string | null): boolean {
		return key !== null && this.remembered(key) !== undefined;
	}

	/** Makes the folder the active tab shows forget its choices. */
	resetActive(): Promise<void> {
		return this.reset(this.input.activeKey);
	}

	/** Makes the folder at `key` forget all its choices: view, sort, grouping, hidden files and icon size. */
	async reset(key: string | null): Promise<void> {
		const handle = this.active;
		if (!handle || key === null) return;
		if (this.pendingSize?.key === key) {
			clearTimeout(this.pendingSize.timer);
			this.pendingSize = null;
		}
		await handle.reset(key).catch(warn('the reset'));
	}
}

function windowView(state: { mode: ViewMode; showHidden: boolean; gridSize: number }): WindowView {
	return { mode: state.mode, showHidden: state.showHidden, gridSize: state.gridSize };
}

function sameWindowView(a: WindowView, b: WindowView): boolean {
	return a.mode === b.mode && a.showHidden === b.showHidden && a.gridSize === b.gridSize;
}

function warn(what: string): (error: unknown) => void {
	return (error) => console.warn(`could not remember ${what} for the folder`, error);
}
