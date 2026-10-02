// The pure rules of thumbnails in the views: what is worth asking for, how big, and in what order
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { ThumbSize } from '@liminal-hq/plugin-thumbnails';

/** Icon groups no generator makes anything of: asking would only fill the failure cache. */
const NO_THUMBNAIL: ReadonlySet<IconGroup> = new Set(['folder', 'audio', 'archive', 'code']);

/**
 * Whether an entry is worth asking the plugin about: a file (or a link to one) of a kind that can
 * have a picture. Folders keep their icon (folder peeks are a later step), as do sounds, archives
 * and source files.
 */
export function wantsThumbnail(entry: Pick<Entry, 'kind' | 'linkTarget' | 'group'>): boolean {
	if (entry.kind === 'directory' || entry.kind === 'other') return false;
	if (entry.kind === 'symlink' && entry.linkTarget !== 'file') return false;
	return !NO_THUMBNAIL.has(entry.group);
}

/**
 * The page's key for an entry's thumbnail. It holds the modified time, so a file that changed is
 * a new key and is asked for again, and an old picture is never shown for new content.
 */
export function entryThumbKey(entry: Pick<Entry, 'id' | 'modifiedMs'>): string {
	return `${entry.id}:${entry.modifiedMs ?? 0}`;
}

/** The standard's sizes, in pixels, smallest first. */
const SIZES: ReadonlyArray<readonly [ThumbSize, number]> = [
	['normal', 128],
	['large', 256],
	['x-large', 512],
];

/** The smallest standard size that holds `cssPixels` on a screen of `pixelRatio`, so a thumbnail is never scaled up. */
export function thumbSizeFor(cssPixels: number, pixelRatio = 1): ThumbSize {
	const needed = Math.ceil(cssPixels * Math.max(1, pixelRatio));
	return SIZES.find(([, pixels]) => needed <= pixels)?.[0] ?? 'xx-large';
}

/** A list row is tall enough for a picture at this height, in pixels (the Roomy density and touch mode). */
export const LIST_THUMBNAIL_MIN_ROW = 40;

/** The pixels a list row's picture takes: the row less its padding. */
export function listThumbnailPixels(rowHeight: number): number {
	return Math.max(0, Math.round(rowHeight - 8));
}

/** Whether the list shows pictures at `rowHeight`. */
export function listShowsThumbnails(rowHeight: number): boolean {
	return rowHeight >= LIST_THUMBNAIL_MIN_ROW;
}

/**
 * The positions to ask for, in the order to ask: what is in view first, then one screen after it
 * (which a scroll down reaches first), then one screen before it nearest first. Positions past
 * either end of the listing are left out.
 */
export function thumbnailPositions(
	first: number,
	last: number,
	count: number,
): { visible: number[]; margin: number[] } {
	if (count <= 0 || last < first) return { visible: [], margin: [] };
	const start = Math.max(0, first);
	const end = Math.min(count - 1, last);
	const screen = end - start + 1;
	const visible = range(start, end);
	const after = range(end + 1, Math.min(count - 1, end + screen));
	const before = range(Math.max(0, start - screen), start - 1).reverse();
	return { visible, margin: [...after, ...before] };
}

function range(from: number, to: number): number[] {
	const out: number[] = [];
	for (let at = from; at <= to; at += 1) out.push(at);
	return out;
}
