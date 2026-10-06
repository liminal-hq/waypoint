// Guards the grid's scroll cost over 100,000 entries: bounded tiles, recycled rows, and only changed cells drawn again
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FOLDER, hugeClient, stubLayout } from '../test/browseHarness';
import { GridView } from './GridView';
import { useListingSession } from './useListingSession';
import { useVfsClient, VfsClientProvider } from './VfsClientContext';

// Every render of a grid cell renders its thumbnail frame once, so counting those counts cell renders.
const renders = vi.hoisted(() => ({ count: 0 }));
vi.mock('../thumbnails/Thumbnail', async (importOriginal) => {
	const actual = await importOriginal<typeof import('../thumbnails/Thumbnail')>();
	return {
		...actual,
		Thumbnail: (props: Parameters<typeof actual.Thumbnail>[0]) => {
			renders.count += 1;
			return actual.Thumbnail(props);
		},
	};
});

// 400 by 600 at 96 px icons: cells of 120 by 160, so four columns and two and a half rows in view.
const COLUMNS = 4;
const ROW = 160;
let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(400, 600);
	renders.count = 0;
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

function Host() {
	const state = useListingSession(useVfsClient(), FOLDER);
	return <GridView state={state} size={96} />;
}

const options = () => screen.queryAllByRole('option');
const placeholders = () => document.querySelectorAll('[role="option"][data-placeholder]').length;

/** Scrolls by `px`, lets the scroll's render and any page it asked for land, and waits for the items. */
async function scrollBy(scroller: HTMLElement, px: number) {
	scroller.scrollTop += px;
	await act(async () => {
		fireEvent.scroll(scroller);
		await Promise.resolve();
	});
	await waitFor(() => expect(placeholders()).toBe(0));
}

describe('scrolling the grid over 100,000 entries', () => {
	it('mounts a bounded number of tiles, builds no elements and redraws only the row that changed', async () => {
		render(
			<VfsClientProvider client={hugeClient(100_000)}>
				<Host />
			</VfsClientProvider>,
		);
		await screen.findByText('entry-0.txt');
		await waitFor(() => expect(placeholders()).toBe(0));
		const listbox = screen.getByRole('listbox');
		const scroller = listbox.parentElement!;
		// Virtualised: the rows in view and a couple either side, not 25,000 rows of four.
		expect(options().length).toBeLessThanOrEqual(COLUMNS * 10);
		expect(options()[0]).toHaveAttribute('aria-setsize', '100000');
		// The glyph samples the icon pictures are coloured from take no room.
		expect(document.querySelector<HTMLElement>('[data-glyph-sample]')!.style.display).toBe('none');

		// The first steps settle the row pool; after that a step adds and removes no elements.
		for (let step = 0; step < 4; step++) await scrollBy(scroller, ROW);
		const mounted = options().length;
		// No element anywhere in the grid is created on a step: not a row, not a cell, and not the
		// parts of a cell as its placeholder fills in.
		const added: Node[] = [];
		const observer = new MutationObserver((records) => {
			for (const record of records) {
				for (const node of record.addedNodes)
					if (node.nodeType === Node.ELEMENT_NODE) added.push(node);
			}
		});
		observer.observe(listbox, { childList: true, subtree: true });

		for (let step = 0; step < 10; step++) {
			renders.count = 0;
			await scrollBy(scroller, ROW);
			// One row changed hands: its cells are drawn for the placeholders and again for the items,
			// and no other cell is drawn at all.
			expect(renders.count).toBeLessThanOrEqual(COLUMNS * 2);
			expect(options().length).toBe(mounted);
		}
		observer.disconnect();
		expect(added).toHaveLength(0);
		expect(screen.getByText(`entry-${(14 + 2) * COLUMNS}.txt`)).toBeInTheDocument();
	});

	it('draws a fling as placeholders from its second jump, and the items once it stops', async () => {
		render(
			<VfsClientProvider client={hugeClient(100_000)}>
				<Host />
			</VfsClientProvider>,
		);
		await screen.findByText('entry-0.txt');
		const scroller = screen.getByRole('listbox').parentElement!;
		const jump = async () => {
			scroller.scrollTop += ROW * 200;
			await act(async () => {
				fireEvent.scroll(scroller);
				await Promise.resolve();
			});
		};
		// A single jump draws what it lands on.
		await jump();
		await waitFor(() => expect(placeholders()).toBe(0));
		// The second, straight after, is a fling: every cell is a placeholder, loaded or not.
		await jump();
		expect(placeholders()).toBe(options().length);
		// Once the scroll has stopped, the items are drawn again.
		await waitFor(() => expect(placeholders()).toBe(0), { timeout: 2000 });
		expect(screen.getByText(`entry-${400 * COLUMNS}.txt`)).toBeInTheDocument();
	});
});
