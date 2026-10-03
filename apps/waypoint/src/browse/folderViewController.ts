// Decides what each folder shows: its own remembered view and sort, or the window's, and writes the choices made in it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A folder remembers only what was chosen in it (SPEC 5.3b). Everything else is the window's: its
// starting sort and grouping, and the view it started in. The window's choices change only while
// remembering is off, so turning it off, and on again, always has the same meaning: a folder with
// a remembered choice shows it, and every other folder shows the window's.
//
// The sort is each listing's own (Rust sorts it), so it is applied when a listing opens and,
// afterwards, through `applySorts`. The view mode is the window's one value, so it follows the
// folder in the active tab.

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import { isOverviewLocation } from '../overview/overviewLocation';
import { isTrashLocation } from '../trash/trashLocation';
import type { FolderViewsHandle } from './folderViewStore';
import type { ListingManager } from './listingManager';
import { sameSort, type ViewMode, type ViewStore } from './viewStore';

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

export class FolderViewController {
	private input: FolderViewInput = { handle: null, enabled: false, activeKey: null };
	/** The mode folders without one of their own show: the window's, changed only while remembering is off. */
	private baselineMode: ViewMode;
	/** Set while the controller itself changes the mode, so the change is not mistaken for a choice. */
	private applying = false;

	constructor(private viewStore: ViewStore) {
		this.baselineMode = viewStore.getState().mode;
	}

	configure(input: FolderViewInput): void {
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

	/** Follows the window's view mode: a change made by the person is remembered by the folder, or by the window. */
	followMode(): () => void {
		let last = this.viewStore.getState().mode;
		return this.viewStore.subscribe((state) => {
			if (state.mode === last) return;
			last = state.mode;
			if (this.applying) return;
			const handle = this.active;
			const key = this.input.activeKey;
			if (handle && key) handle.remember(key, { mode: state.mode }).catch(warn('the view'));
			else if (!handle) this.baselineMode = state.mode;
		});
	}

	/** Brings the window's mode and every open listing's sort in line with what the folders remember. */
	reconcile(manager: ListingManager): void {
		const handle = this.active;
		// Rust has not answered this window's own writes: what is on screen is the newer choice.
		if (handle && handle.store.getState().writing > 0) return;
		this.reconcileMode();
		manager.applySorts(this.sortFor);
	}

	private reconcileMode(): void {
		const key = this.input.activeKey;
		let wanted: ViewMode;
		if (!this.active) wanted = this.baselineMode;
		// The Trash and Overview keep the mode they have.
		else if (key) wanted = this.remembered(key)?.mode ?? this.baselineMode;
		else return;
		if (this.viewStore.getState().mode === wanted) return;
		this.applying = true;
		try {
			this.viewStore.getState().setMode(wanted);
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

	/** Makes the folder at `key` forget its choices. */
	async reset(key: string | null): Promise<void> {
		const handle = this.active;
		if (!handle || key === null) return;
		await handle.reset(key).catch(warn('the reset'));
	}
}

function warn(what: string): (error: unknown) => void {
	return (error) => console.warn(`could not remember ${what} for the folder`, error);
}
