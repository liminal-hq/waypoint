// Tells a loader which items a view shows, each time the visible range or the listing changes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useRef } from 'react';
import type { ThumbnailLoader } from './thumbnailLoader';
import { thumbnailPositions } from './thumbnailModel';

interface ViewportThumbnails<Item extends { key: string }> {
	loader: ThumbnailLoader<Item> | null;
	/** The first and last positions in view (not the rows drawn beyond the edge). */
	first: number;
	last: number;
	count: number;
	/** The item to ask for at a position, or `null` for one with no thumbnail to ask for or not loaded yet. */
	itemAt(position: number): Item | null;
	/** Changes when the listing's entries do, so positions that were not loaded are looked at again. */
	version: number;
	/**
	 * While the range keeps changing (a scroll), wait until it has stayed put for this many
	 * milliseconds and ask for what is in view then; a change after a quiet spell is asked for at once.
	 * 0 (the default) asks at every change. A grid scrolled fast passes a few hundred items a frame,
	 * and asking for each of them (and hearing back about each), only to withdraw them a frame later,
	 * cost a fifth of every frame (#565).
	 */
	settleMs?: number;
	/** The view is being scrolled: with `settleMs`, nothing is asked for until the range settles. */
	scrolling?: boolean;
}

/**
 * Asks for what is in view and a screen either side, and withdraws what scrolled out of that:
 * everything the loader holds goes when the view unmounts (`useEntryThumbnailLoader` disposes it).
 */
export function useViewportThumbnails<Item extends { key: string }>({
	loader,
	first,
	last,
	count,
	itemAt,
	version,
	settleMs = 0,
	scrolling = false,
}: ViewportThumbnails<Item>): void {
	const lookup = useRef(itemAt);
	lookup.current = itemAt;
	const latest = useRef({ first, last, count });
	latest.current = { first, last, count };
	const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const changed = useRef(Number.NEGATIVE_INFINITY);

	useEffect(() => {
		if (!loader) return;
		const send = () => {
			const range = latest.current;
			const { visible, margin } = thumbnailPositions(range.first, range.last, range.count);
			const items = (positions: number[]) =>
				positions.flatMap((position) => {
					const item = lookup.current(position);
					return item ? [item] : [];
				});
			loader.want(items(visible), items(margin));
		};
		const now = Date.now();
		const quiet = now - changed.current >= settleMs;
		changed.current = now;
		if (settleMs <= 0 || (quiet && !scrolling && timer.current === null)) {
			send();
			return;
		}
		// Still moving: ask once the range has stayed put, for what is in view by then.
		if (timer.current !== null) clearTimeout(timer.current);
		timer.current = setTimeout(() => {
			timer.current = null;
			send();
		}, settleMs);
	}, [loader, first, last, count, version, settleMs, scrolling]);

	// A request still waiting goes with the view (or with a loader that changed).
	useEffect(
		() => () => {
			if (timer.current !== null) clearTimeout(timer.current);
			timer.current = null;
		},
		[loader],
	);
}
