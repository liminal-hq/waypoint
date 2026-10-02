// Verifies group headers in the list and the grid: counts, folding, the keyboard, and what a screen reader hears
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { FakeVfsClient, makeEntry, syntheticEntries } from '../services/fakeVfsClient';
import type { VfsClient } from '../services/vfsClient';
import { FOLDER, stubLayout, withOverrides } from '../test/browseHarness';
import { GridView } from './GridView';
import { ListView } from './ListView';
import { useListingSession } from './useListingSession';
import { useVfsClient, VfsClientProvider } from './VfsClientContext';

const BY_SIZE: SortSpec = {
	key: 'name',
	descending: false,
	directoriesFirst: true,
	groupBy: 'size',
};

/** Two folders (no size), one empty file, two tiny ones and two large ones, in no particular order. */
function folder(): Entry[] {
	return [
		makeEntry(1, 'src', { kind: 'directory' }),
		makeEntry(2, 'b.txt', { size: 7 }),
		makeEntry(3, 'big.bin', { size: 3_000_000 }),
		makeEntry(4, 'docs', { kind: 'directory' }),
		makeEntry(5, 'a.txt', { size: 5 }),
		makeEntry(6, 'empty', { size: 0 }),
		makeEntry(7, 'huge.bin', { size: 2_000_000 }),
	];
}

function grouped(entries = folder()): { client: VfsClient; fake: FakeVfsClient } {
	const fake = new FakeVfsClient();
	fake.setFolder(FOLDER, entries);
	const client = withOverrides(fake, {
		openListing: (location, options) => fake.openListing(location, { ...options, sort: BY_SIZE }),
	});
	return { client, fake };
}

function Grid({ client }: { client: VfsClient }) {
	const state = useListingSession(useVfsClient(), FOLDER);
	void client;
	return <GridView state={state} size={96} />;
}

const headers = () => screen.queryAllByRole('group', { name: /(expanded|collapsed)$/ });
const names = () => screen.queryAllByRole('option').map((o) => o.textContent?.split(/\d/)[0]);
const listbox = () => screen.getByRole('listbox');
const active = () => listbox().getAttribute('aria-activedescendant');

function press(key: string, init: KeyboardEventInit = {}) {
	fireEvent.keyDown(listbox(), { key, ...init });
}

describe('the list with groups', () => {
	let restore: () => void;
	beforeEach(() => {
		restore = stubLayout(400);
	});
	afterEach(() => {
		cleanup();
		restore();
	});

	async function renderList(client: VfsClient) {
		render(
			<VfsClientProvider client={client}>
				<ListView location={FOLDER} />
			</VfsClientProvider>,
		);
		await screen.findByRole('listbox');
		await waitFor(() => expect(headers().length).toBeGreaterThan(0));
		await waitFor(() => expect(screen.queryAllByRole('option', { busy: true })).toHaveLength(0));
	}

	it('puts a header with its item count ahead of each group, named for a screen reader', async () => {
		await renderList(grouped().client);
		expect(headers().map((h) => h.getAttribute('aria-label'))).toEqual([
			'Unspecified, 2 items, expanded',
			'Empty, 1 item, expanded',
			'Tiny (under 10 kB), 2 items, expanded',
			'Large (1 to 16 MB), 2 items, expanded',
		]);
		expect(headers()[2]).toHaveTextContent('2 items');
		// Entries keep their place among the entries: a header is not one of them.
		expect(screen.getAllByRole('option')).toHaveLength(7);
		expect(screen.getAllByRole('option')[2]).toHaveAttribute('aria-posinset', '3');
		expect(screen.getAllByRole('option')[2]).toHaveAttribute('aria-setsize', '7');
	});

	it('folds a group shut and open again with a click on its header', async () => {
		await renderList(grouped().client);
		fireEvent.click(headers()[2]!);
		expect(headers()[2]).toHaveAttribute('aria-label', 'Tiny (under 10 kB), 2 items, collapsed');
		expect(headers()[2]).toHaveAttribute('data-collapsed');
		expect(screen.queryByRole('option', { name: /a\.txt/ })).toBeNull();
		expect(screen.getAllByRole('option')).toHaveLength(5);
		fireEvent.click(headers()[2]!);
		expect(screen.getAllByRole('option')).toHaveLength(7);
	});

	it('is stepped through by the arrow keys without stopping on a header', async () => {
		await renderList(grouped().client);
		act(() => listbox().focus());
		await waitFor(() => expect(active()).toMatch(/-row-0$/));
		press('ArrowDown');
		await waitFor(() => expect(active()).toMatch(/-row-1$/));
		// Past the last folder the next stop is the first entry of the next group, not its header.
		press('ArrowDown');
		await waitFor(() => expect(active()).toMatch(/-row-2$/));
		press('ArrowUp');
		await waitFor(() => expect(active()).toMatch(/-row-1$/));
		press('End');
		await waitFor(() => expect(active()).toMatch(/-row-6$/));
		press('Home');
		await waitFor(() => expect(active()).toMatch(/-row-0$/));
	});

	it('steps onto a header with Left, folds with Left and opens with Right', async () => {
		await renderList(grouped().client);
		act(() => listbox().focus());
		press('ArrowDown');
		press('ArrowDown');
		press('ArrowDown');
		await waitFor(() => expect(active()).toMatch(/-row-3$/));
		press('ArrowLeft');
		await waitFor(() => expect(active()).toMatch(/-group-2$/));
		press('ArrowLeft');
		await waitFor(() => expect(headers()[2]).toHaveAttribute('data-collapsed'));
		press('ArrowRight');
		await waitFor(() => expect(headers()[2]).not.toHaveAttribute('data-collapsed'));
		press('Enter');
		await waitFor(() => expect(headers()[2]).toHaveAttribute('data-collapsed'));
		// A folded group is not an entry the keyboard can land on, but its header is a stop.
		press('ArrowUp');
		await waitFor(() => expect(active()).toMatch(/-row-2$/));
	});

	it('does not offer the rows of a folded group to the keyboard, and stops at its header', async () => {
		await renderList(grouped().client);
		fireEvent.click(headers()[1]!);
		act(() => listbox().focus());
		await waitFor(() => expect(active()).toBeTruthy());
		press('Home');
		press('ArrowDown');
		press('ArrowDown');
		// Two folders, then the folded Empty group's header.
		await waitFor(() => expect(active()).toMatch(/-group-1$/));
		press('ArrowDown');
		await waitFor(() => expect(active()).toMatch(/-row-3$/));
		expect(screen.getAllByRole('option')[2]).toHaveTextContent('a.txt');
	});

	it('regroups when the sort changes and moves a row that changes group with its header', async () => {
		const { client, fake } = grouped();
		await renderList(client);
		expect(headers()).toHaveLength(4);
		// a.txt grows from tiny to large: the tiny group keeps one row and the large one gains it.
		fake.updateEntries(FOLDER, new Map([[5, { size: 4_000_000 }]]));
		await waitFor(() =>
			expect(headers().map((h) => h.getAttribute('aria-label'))).toEqual([
				'Unspecified, 2 items, expanded',
				'Empty, 1 item, expanded',
				'Tiny (under 10 kB), 1 item, expanded',
				'Large (1 to 16 MB), 3 items, expanded',
			]),
		);
		fake.updateEntries(FOLDER, new Map([[2, { size: 4_000_000 }]]));
		await waitFor(() => expect(headers()).toHaveLength(3));
		expect(names()).toHaveLength(7);
	});
});

describe('the grid with groups', () => {
	let restore: () => void;
	beforeEach(() => {
		// 600 px wide: four columns of 96 px icons.
		restore = stubLayout(600, 600);
	});
	afterEach(() => {
		cleanup();
		restore();
	});

	async function renderGrid(client: VfsClient) {
		render(
			<VfsClientProvider client={client}>
				<Grid client={client} />
			</VfsClientProvider>,
		);
		await screen.findByRole('listbox');
		await waitFor(() => expect(headers().length).toBeGreaterThan(0));
		await waitFor(() => expect(screen.queryAllByRole('option', { busy: true })).toHaveLength(0));
	}

	it('draws each header across the grid with its rows of cells under it', async () => {
		await renderGrid(grouped().client);
		expect(headers().map((h) => h.getAttribute('aria-label'))).toEqual([
			'Unspecified, 2 items, expanded',
			'Empty, 1 item, expanded',
			'Tiny (under 10 kB), 2 items, expanded',
			'Large (1 to 16 MB), 2 items, expanded',
		]);
		expect(screen.getAllByRole('option')).toHaveLength(7);
	});

	it('folds a group with a click, hides its cells and keeps the others', async () => {
		await renderGrid(grouped().client);
		fireEvent.click(headers()[3]!);
		expect(headers()[3]).toHaveAttribute('data-collapsed');
		expect(screen.queryByRole('option', { name: /big\.bin/ })).toBeNull();
		expect(screen.getAllByRole('option')).toHaveLength(5);
	});

	it('moves along the cells past a header, and steps onto a header from the first cell of a group', async () => {
		await renderGrid(grouped().client);
		act(() => listbox().focus());
		await waitFor(() => expect(active()).toMatch(/-item-0$/));
		press('ArrowRight');
		await waitFor(() => expect(active()).toMatch(/-item-1$/));
		// The next cell is in the next group: the header between them is skipped.
		press('ArrowRight');
		await waitFor(() => expect(active()).toMatch(/-item-2$/));
		// The first cell of a group steps up onto the group's header, and Left there folds it.
		press('ArrowLeft');
		await waitFor(() => expect(active()).toMatch(/-group-1$/));
		press('ArrowLeft');
		await waitFor(() => expect(headers()[1]).toHaveAttribute('data-collapsed'));
		expect(screen.queryByRole('option', { name: /empty/ })).toBeNull();
	});
});

describe('virtual scrolling with groups', () => {
	let restore: () => void;
	beforeEach(() => {
		restore = stubLayout(280);
	});
	afterEach(() => {
		cleanup();
		restore();
	});

	it('draws only the rows near the viewport, headers among them, and counts entries only', async () => {
		const fake = new FakeVfsClient();
		fake.setFolder(FOLDER, syntheticEntries(5000));
		const client = withOverrides(fake, {
			openListing: (location, options) =>
				fake.openListing(location, {
					...options,
					sort: { ...BY_SIZE, groupBy: 'type' },
				}),
		});
		render(
			<VfsClientProvider client={client}>
				<ListView location={FOLDER} />
			</VfsClientProvider>,
		);
		await screen.findByRole('listbox');
		await waitFor(() => expect(headers().length).toBeGreaterThan(0));
		const drawn = screen.getAllByRole('option').length + headers().length;
		expect(drawn).toBeLessThan(40);
		expect(headers()[0]).toHaveAttribute('aria-label', expect.stringMatching(/^Folders, /));
		const first = screen.getAllByRole('option')[0]!;
		expect(first).toHaveAttribute('aria-posinset', '1');
		expect(first).toHaveAttribute('aria-setsize', '5000');
		// Headers are not rows of the list's count.
		expect(screen.getByRole('listbox')).not.toHaveAttribute('aria-rowcount');
	});
});
