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
}: ViewportThumbnails<Item>): void {
	const lookup = useRef(itemAt);
	lookup.current = itemAt;
	useEffect(() => {
		if (!loader) return;
		const { visible, margin } = thumbnailPositions(first, last, count);
		const items = (positions: number[]) =>
			positions.flatMap((position) => {
				const item = lookup.current(position);
				return item ? [item] : [];
			});
		loader.want(items(visible), items(margin));
	}, [loader, first, last, count, version]);
}
