// Finding the entries a command just made in a listing, so they can be selected and put into rename
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { PAGE_SIZE, type ListingModel } from './listingModel';
import { isReset, mapPosition } from './patch';
import { isSelected, type Selection } from './selection';
import type { BrowseStore } from './browseStore';

/**
 * The position of the entry called exactly `name` (names are compared as written: Rust, not the
 * view, decides what clashes), reading the listing a page at a time. `null` when there is none.
 */
export async function findByName(model: ListingModel, name: string): Promise<number | null> {
	const total = model.count;
	for (let position = 0; position < total;) {
		const end = Math.min(total, (Math.floor(position / PAGE_SIZE) + 1) * PAGE_SIZE);
		const rows = await model.readRange(position, end);
		const found = rows.findIndex((entry) => entry.name === name);
		if (found >= 0) return position + found;
		position = end;
	}
	return null;
}

/** How long to wait for the listing to show something a job made, which its watcher reports a moment later. */
export const REVEAL_TIMEOUT_MS = 4000;

/** Resolves on the model's next change, or after `ms`, whichever is first. */
function nextChange(model: ListingModel, ms: number): Promise<void> {
	return new Promise((resolve) => {
		let stop = () => {};
		const timer = setTimeout(() => {
			stop();
			resolve();
		}, ms);
		stop = model.subscribe(() => {
			clearTimeout(timer);
			stop();
			resolve();
		});
	});
}

export interface RevealOptions {
	/** Put the entry's name into inline rename once it is selected. */
	rename?: boolean;
	timeoutMs?: number;
}

/**
 * Waits for an entry called `name` to appear in the listing, then selects it, focuses it, scrolls
 * it into sight and, with `rename`, puts its name into inline rename. Resolves to whether it
 * appeared in time.
 */
export async function revealByName(
	model: ListingModel,
	store: BrowseStore,
	name: string,
	options: RevealOptions = {},
): Promise<boolean> {
	const deadline = Date.now() + (options.timeoutMs ?? REVEAL_TIMEOUT_MS);
	for (;;) {
		const position = await findByName(model, name);
		if (position !== null) {
			const entry = model.entryAt(position) ?? (await model.readRange(position, position + 1))[0];
			if (entry) {
				const state = store.getState();
				state.selectEntries([entry.id], position);
				state.requestScroll(position);
				if (options.rename) state.beginRename(entry.id);
				return true;
			}
		}
		const left = deadline - Date.now();
		if (left <= 0) return false;
		await nextChange(model, left);
	}
}

/**
 * Follows the positions the listing inserts entries at, through every later patch, so a command
 * that makes entries it cannot name (a duplicate gets whatever name is free) can find them: they
 * are the entries inserted while it ran. A reset (a re-sort, a rescan) loses the positions.
 */
export class InsertTracker {
	private positions: number[] = [];
	private lost = false;
	private stop: () => void;

	constructor(private model: ListingModel) {
		this.stop = model.onPatch((report) => {
			if (isReset(report.ops)) {
				this.lost = true;
				return;
			}
			// Each op is in the coordinates the ones before it leave, so each moves what is known.
			for (const op of report.ops) {
				this.positions = this.positions.flatMap((position) => {
					const mapped = mapPosition(position, [op]);
					return mapped.removed ? [] : [mapped.position];
				});
				if (op.kind === 'insert') {
					for (let offset = 0; offset < op.count; offset++) this.positions.push(op.at + offset);
				}
			}
		});
	}

	/** How many inserted entries are being followed. */
	get count(): number {
		return this.lost ? 0 : this.positions.length;
	}

	/** The inserted entries now, in listing order, or `[]` when a reset lost them. */
	async entries(): Promise<Array<{ position: number; entry: Entry }>> {
		if (this.lost) return [];
		const found: Array<{ position: number; entry: Entry }> = [];
		for (const position of [...this.positions].sort((a, b) => a - b)) {
			const entry = (await this.model.readRange(position, position + 1))[0];
			if (entry) found.push({ position, entry });
		}
		return found;
	}

	dispose(): void {
		this.stop();
	}
}

/** Waits until `tracker` holds `expected` entries or `timeoutMs` passes. */
export async function waitForInserts(
	model: ListingModel,
	tracker: InsertTracker,
	expected: number,
	timeoutMs = REVEAL_TIMEOUT_MS,
): Promise<void> {
	const deadline = Date.now() + timeoutMs;
	while (tracker.count < expected) {
		const left = deadline - Date.now();
		if (left <= 0) return;
		await nextChange(model, left);
	}
}

/** Selects the tracked entries, focuses the first and scrolls to it. Returns how many it found. */
export async function revealInserted(store: BrowseStore, tracker: InsertTracker): Promise<number> {
	const found = await tracker.entries();
	const first = found[0];
	if (!first) return 0;
	const state = store.getState();
	state.selectEntries(
		found.map(({ entry }) => entry.id),
		first.position,
	);
	state.requestScroll(first.position);
	return found.length;
}

/** The names of up to `limit` selected entries, found by reading the listing a page at a time from the top. */
export async function firstSelectedNames(
	model: ListingModel,
	selection: Selection,
	limit: number,
): Promise<string[]> {
	const names: string[] = [];
	const total = model.count;
	for (let position = 0; position < total && names.length < limit;) {
		const end = Math.min(total, (Math.floor(position / PAGE_SIZE) + 1) * PAGE_SIZE);
		for (const entry of await model.readRange(position, end)) {
			if (isSelected(selection, entry.id)) {
				names.push(entry.name);
				if (names.length >= limit) break;
			}
		}
		position = end;
	}
	return names;
}
