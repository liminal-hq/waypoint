// Type-ahead: jump to the next entry whose name starts with what was typed, even if its page is not loaded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { PAGE_SIZE, type ListingModel } from './listingModel';

/** How long a pause ends a typed prefix and starts a new one. */
export const TYPE_AHEAD_RESET_MS = 800;

/** Accumulates typed characters into a prefix that resets after a pause. */
export class TypeAheadBuffer {
	private text = '';
	private last = 0;

	constructor(
		private resetMs = TYPE_AHEAD_RESET_MS,
		private now: () => number = () => performance.now(),
	) {}

	/** Adds a character and returns the prefix so far, starting over if the last one was a while ago. */
	push(character: string): string {
		const at = this.now();
		if (at - this.last > this.resetMs) this.text = '';
		this.last = at;
		this.text += character;
		return this.text;
	}

	/** Whether a prefix is being typed now, so a space belongs to it rather than meaning Space. */
	get active(): boolean {
		return this.text !== '' && this.now() - this.last <= this.resetMs;
	}

	reset(): void {
		this.text = '';
	}
}

/**
 * Finds the first position at or after `from` (wrapping round to the start) whose name begins with
 * `prefix`, ignoring case. It goes through the model a page at a time, so it reaches entries the
 * view never loaded; `cancelled` is polled between pages so a newer keystroke can abandon a scan
 * of a huge folder. Resolves to `null` when nothing matches or the scan was cancelled.
 */
export async function findByPrefix(
	model: ListingModel,
	prefix: string,
	from: number,
	cancelled: () => boolean = () => false,
): Promise<number | null> {
	const total = model.count;
	if (total === 0 || prefix === '') return null;
	const needle = prefix.toLocaleLowerCase();
	const start = from >= total ? 0 : Math.max(0, from);
	const spans: Array<[number, number]> = [
		[start, total],
		[0, start],
	];
	for (const [low, high] of spans) {
		let position = low;
		while (position < high) {
			if (cancelled()) return null;
			const end = Math.min(high, (Math.floor(position / PAGE_SIZE) + 1) * PAGE_SIZE);
			const rows = await model.readRange(position, end);
			if (cancelled()) return null;
			for (let offset = 0; offset < rows.length; offset++) {
				if (rows[offset]!.name.toLocaleLowerCase().startsWith(needle)) return position + offset;
			}
			position = end;
		}
	}
	return null;
}
