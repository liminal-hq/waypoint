// Verifies the list view: rows and roles, placeholders, sorting, the loading, scanning and error states, and the scroll cap
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import type { ListingSnapshot } from '@liminal-hq/waypoint-protocol/generated/ListingSnapshot';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import type { VfsClient } from '../services/vfsClient';
import { clientWith, FOLDER, stubLayout } from '../test/browseHarness';
import { ListView } from './ListView';
import { VfsClientProvider } from './VfsClientContext';

let restoreLayout: () => void;
beforeEach(() => {
	// 280 px shows ten 28 px rows.
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

function renderList(client: VfsClient) {
	return render(
		<VfsClientProvider client={client}>
			<ListView location={FOLDER} />
		</VfsClientProvider>,
	);
}

const rows = () => screen.queryAllByRole('option');

/**
 * Wraps a client so `getRange` waits until `release()` is called. A proxy, so a method added to the
 * client later is forwarded without this helper knowing about it.
 */
function holdRanges(client: VfsClient): { held: VfsClient; release: () => void } {
	let release: () => void = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	const held = new Proxy(client, {
		get(target, property) {
			const value = Reflect.get(target, property, target) as unknown;
			if (property === 'getRange') {
				return async (...args: Parameters<VfsClient['getRange']>) => {
					await gate;
					return target.getRange(...args);
				};
			}
			return typeof value === 'function' ? value.bind(target) : value;
		},
	});
	return { held, release };
}

/** A client that reports a huge folder without holding it, to test the scroll cap. */
function hugeClient(count: number): VfsClient {
	const snapshot: ListingSnapshot = {
		handle: 1,
		location: FOLDER,
		revision: 1,
		count,
		phase: 'ready',
		sort: { key: 'name', descending: false, directoriesFirst: true },
		filter: { showHidden: false },
	};
	return {
		openListing: async () => snapshot,
		getRange: async (_handle, start, length) =>
			Array.from({ length: Math.min(length, count - start) }, (_, i) =>
				makeEntry(start + i, `entry-${start + i}.txt`),
			),
		setSort: async () => snapshot,
		setFilter: async () => snapshot,
		closeListing: async () => {},
		onListingEvent: () => () => {},
	};
}

describe('listing', () => {
	it('opens the folder and shows its rows as listbox options', async () => {
		const { client } = clientWith(1000);
		renderList(client);
		expect(screen.getByRole('status')).toHaveTextContent('Opening folder…');
		const list = await screen.findByRole('listbox', { name: 'Files' });
		expect(list).toHaveAttribute('aria-multiselectable', 'true');
		expect(list).toHaveAttribute('aria-rowcount', '1000');
		expect(screen.queryByRole('grid')).toBeNull();
		await waitFor(() =>
			expect(within(list).getAllByText(/^(folder|file)-/).length).toBeGreaterThan(5),
		);
	});

	it('draws only the rows near the viewport, each naming its place in the whole list', async () => {
		const { client } = clientWith(100_000);
		renderList(client);
		await screen.findByRole('listbox');
		await waitFor(() => expect(rows().length).toBeGreaterThan(0));
		expect(rows().length).toBeLessThan(40);
		const first = rows()[0]!;
		expect(first).toHaveAttribute('aria-setsize', '100000');
		expect(first).toHaveAttribute('aria-posinset', '1');
		expect(rows()[3]).toHaveAttribute('aria-posinset', '4');
	});

	it('shows placeholder rows while a page loads, then the entries', async () => {
		// The page is held until the test lets it go, so the placeholder state is there to see however
		// slow the runner is. (A simulated latency would race the assertion.)
		const { client } = clientWith(1000);
		const { held, release } = holdRanges(client);
		renderList(held);
		await screen.findByRole('listbox');
		await waitFor(() => expect(rows().length).toBeGreaterThan(0));
		expect(rows()[0]).toHaveAttribute('data-placeholder');
		expect(rows()[0]).toHaveAttribute('aria-busy', 'true');
		release();
		await waitFor(() => expect(rows()[0]).not.toHaveAttribute('data-placeholder'));
		expect(rows()[0]).not.toHaveAttribute('aria-busy');
	});

	it('shows each entry with its size, modified time and kind', async () => {
		const client = new FakeVfsClient();
		client.setFolder(FOLDER, [
			makeEntry(1, 'docs', { kind: 'directory' }),
			makeEntry(2, 'photo.jpg', { size: 2_500_000, modifiedMs: Date.UTC(2026, 0, 2, 15, 4) }),
		]);
		renderList(client);
		const photo = await screen.findByRole('option', { name: /photo\.jpg/ });
		expect(photo).toHaveTextContent('2.5');
		expect(photo).toHaveTextContent('2026');
		expect(photo).toHaveTextContent('Image');
		const docs = screen.getByRole('option', { name: /docs/ });
		expect(docs).toHaveTextContent('Folder');
		expect(docs).toHaveTextContent('—');
		expect(docs.querySelector('svg[data-group="folder"]')).not.toBeNull();
		expect(photo.querySelector('svg[data-group="image"]')).not.toBeNull();
	});

	it('shows an empty-folder state', async () => {
		const client = new FakeVfsClient();
		client.setFolder(FOLDER, []);
		renderList(client);
		const empty = await screen.findByText('This folder is empty.');
		expect(empty).toHaveAttribute('role', 'status');
		expect(screen.queryByRole('listbox')).toBeNull();
	});

	it('closes the listing when it goes away', async () => {
		const { client } = clientWith(10);
		const view = renderList(client);
		await screen.findByRole('listbox');
		expect(client.openCount).toBe(1);
		view.unmount();
		expect(client.openCount).toBe(0);
	});
});

describe('error states', () => {
	it.each([
		['notFound', 'Folder not found', /does not exist/],
		['permissionDenied', 'Permission denied', /do not have permission/],
		['notADirectory', 'Not a folder', /is a file, not a folder/],
	] as const)('shows a distinct alert for %s', async (kind, title, detail) => {
		const client = new FakeVfsClient();
		client.failOpening(FOLDER, { kind, location: fileLocation('/home/test') });
		renderList(client);
		const alert = await screen.findByRole('alert');
		expect(alert).toHaveAttribute('data-error', kind);
		expect(within(alert).getByRole('heading', { name: title })).toBeVisible();
		expect(alert).toHaveTextContent(detail);
		expect(alert).toHaveTextContent('/home/test');
		expect(screen.queryByRole('listbox')).toBeNull();
	});

	it('shows a generic alert for any other failure', async () => {
		const client = new FakeVfsClient();
		client.failOpening(FOLDER, { kind: 'io', message: 'disk on fire', location: null });
		renderList(client);
		expect(await screen.findByRole('alert')).toHaveTextContent('This folder could not be shown');
	});

	it('replaces the list with an alert when an open listing fails', async () => {
		const { client } = clientWith(100);
		renderList(client);
		await screen.findByRole('listbox');
		act(() => client.failListing(1, { kind: 'notFound', location: FOLDER }));
		expect(await screen.findByRole('alert')).toHaveAttribute('data-error', 'notFound');
	});
});

describe('scanning', () => {
	it('shows progress while a scan runs, then the final list', async () => {
		const { client } = clientWith(300);
		let emit: (event: ListingEvent) => void = () => {};
		const scanning: VfsClient = {
			openListing: async (location, options) => ({
				...(await client.openListing(location, options)),
				phase: 'scanning',
				count: 120,
			}),
			getRange: (handle, start, count) => client.getRange(handle, start, count),
			setSort: (handle, sort) => client.setSort(handle, sort),
			setFilter: (handle, filter) => client.setFilter(handle, filter),
			closeListing: (handle) => client.closeListing(handle),
			onListingEvent: (listener) => {
				emit = listener;
				return () => {};
			},
		};
		renderList(scanning);
		const notice = await screen.findByText(/Scanning…/);
		expect(notice).toHaveAttribute('role', 'status');
		expect(notice).toHaveTextContent('120 items found so far');
		act(() =>
			emit({
				kind: 'progress',
				handle: 1,
				revision: 1,
				phase: 'scanning',
				scanned: 250,
				count: 250,
			}),
		);
		expect(screen.getByText(/Scanning…/)).toHaveTextContent('250 items');
		act(() =>
			emit({ kind: 'progress', handle: 1, revision: 1, phase: 'ready', scanned: 300, count: 300 }),
		);
		await waitFor(() => expect(screen.queryByText(/Scanning…/)).toBeNull());
		expect(screen.getByRole('listbox')).toHaveAttribute('aria-rowcount', '300');
	});
});

describe('sort header', () => {
	it('sorts by a column, toggles direction on a second click, and reflects it', async () => {
		const { client } = clientWith(500);
		const setSort = vi.spyOn(client, 'setSort');
		renderList(client);
		await screen.findByRole('listbox');
		const name = screen.getByRole('button', { name: /^Name/ });
		expect(name).toHaveAttribute('data-sorted', 'ascending');
		expect(name).toHaveTextContent('sorted ascending');

		fireEvent.click(name);
		await waitFor(() => expect(name).toHaveAttribute('data-sorted', 'descending'));
		expect(setSort).toHaveBeenLastCalledWith(1, {
			key: 'name',
			descending: true,
			directoriesFirst: true,
		});

		fireEvent.click(screen.getByRole('button', { name: /^Size/ }));
		await waitFor(() =>
			expect(screen.getByRole('button', { name: /^Size/ })).toHaveAttribute(
				'data-sorted',
				'ascending',
			),
		);
		expect(name).not.toHaveAttribute('data-sorted');
		expect(setSort).toHaveBeenLastCalledWith(1, {
			key: 'size',
			descending: false,
			directoriesFirst: true,
		});
	});

	it('offers all four columns', async () => {
		const { client } = clientWith(5);
		renderList(client);
		await screen.findByRole('listbox');
		for (const column of ['Name', 'Size', 'Modified', 'Kind']) {
			expect(screen.getByRole('button', { name: new RegExp(`^${column}`) })).toBeVisible();
		}
	});
});

describe('the scroll cap', () => {
	it('shows a banner and lays out no more rows than the cap allows', async () => {
		renderList(hugeClient(2_000_000));
		const list = await screen.findByRole('listbox');
		const banner = await screen.findByText(/Showing the first/);
		expect(banner).toHaveAttribute('role', 'status');
		expect(banner).toHaveTextContent(/1,198,372 of 2,000,000 items/);
		expect(list).toHaveAttribute('aria-rowcount', '1198372');
		expect(list.style.getPropertyValue('--wp-list-height')).toBe(`${1_198_372 * 28}px`);
	});

	it('shows no banner below the cap', async () => {
		// `hugeClient` serves rows on demand; `clientWith` would build 500 000 real entries first,
		// which takes seconds on a slow CI runner.
		renderList(hugeClient(500_000));
		await screen.findByRole('listbox');
		expect(screen.queryByText(/Showing the first/)).toBeNull();
	});

	it('never renders a blank tail: the last row the cap allows is a real entry', async () => {
		renderList(hugeClient(2_000_000));
		const list = await screen.findByRole('listbox');
		const scroller = list.parentElement!;
		act(() => {
			scroller.scrollTop = 1_198_372 * 28;
			fireEvent.scroll(scroller);
		});
		await waitFor(() =>
			expect(screen.getByRole('option', { name: /entry-1198371\.txt/ })).toBeVisible(),
		);
		const last = rows().at(-1)!;
		expect(last).toHaveAttribute('aria-posinset', '1198372');
		expect(last).not.toHaveAttribute('data-placeholder');
	});
});
