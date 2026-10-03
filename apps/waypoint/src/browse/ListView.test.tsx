// Verifies the list view: rows and roles, placeholders, sorting, states, the scroll cap, selection and keyboard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import type { VfsClient } from '../services/vfsClient';
import { clientWith, FOLDER, hugeClient, stubLayout, withOverrides } from '../test/browseHarness';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { ListView } from './ListView';
import { FakeTimeFormatClient } from '../services/fakeTimeFormatClient';
import { TimeFormatProvider } from './TimeFormatContext';
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

function renderList(client: VfsClient, onOpen?: (entry: Entry) => void) {
	return render(
		<VfsClientProvider client={client}>
			<ListView location={FOLDER} onOpen={onOpen} />
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

	it('shows the modified time on the 24-hour clock the system is set to, and follows a change', async () => {
		const client = new FakeVfsClient();
		client.setFolder(FOLDER, [
			makeEntry(1, 'photo.jpg', { size: 10, modifiedMs: new Date(2026, 9, 1, 13, 5).getTime() }),
		]);
		const timeFormat = new FakeTimeFormatClient('h23');
		render(
			<VfsClientProvider client={client}>
				<TimeFormatProvider client={timeFormat}>
					<ListView location={FOLDER} />
				</TimeFormatProvider>
			</VfsClientProvider>,
		);
		const photo = await screen.findByRole('option', { name: /photo\.jpg/ });
		await waitFor(() => expect(photo).toHaveTextContent('13:05'));
		act(() => timeFormat.set('h12'));
		await waitFor(() => expect(photo).not.toHaveTextContent('13:05'));
		expect(photo).toHaveTextContent(/\b1:05/);
		act(() => timeFormat.set('h23'));
		await waitFor(() => expect(photo).toHaveTextContent('13:05'));
	});

	it('measures the row height once the scroller appears after an empty first render', async () => {
		const real = window.getComputedStyle.bind(window);
		const spy = vi.spyOn(window, 'getComputedStyle').mockImplementation((element, pseudo) => {
			const style = real(element, pseudo);
			return new Proxy(style, {
				get(target, property) {
					if (property === 'getPropertyValue') {
						return (name: string) =>
							name === '--wp-row-height' ? '40px' : target.getPropertyValue(name);
					}
					const value = Reflect.get(target, property, target) as unknown;
					return typeof value === 'function' ? value.bind(target) : value;
				},
			});
		});
		try {
			const { client } = clientWith(0);
			renderList(client);
			await screen.findByText('This folder is empty.');
			act(() => client.addEntries(FOLDER, [makeEntry(1, 'a.txt'), makeEntry(2, 'b.txt')]));
			await waitFor(() => expect(rows()).toHaveLength(2));
			expect(rows()[1]!.style.getPropertyValue('--wp-row-y')).toBe('40px');
		} finally {
			spy.mockRestore();
		}
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
		const scanning = withOverrides(client, {
			openListing: async (location, options) => ({
				...(await client.openListing(location, options)),
				phase: 'scanning',
				count: 120,
			}),
			onListingEvent: (listener) => {
				emit = listener;
				return () => {};
			},
		});
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

describe('Alt+arrow keys', () => {
	it('are left to the window so history and up still work', async () => {
		const { client } = clientWith(30);
		renderList(client);
		await screen.findByRole('listbox');
		for (const key of ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']) {
			expect(fireEvent.keyDown(screen.getByRole('listbox'), { key, altKey: true })).toBe(true);
		}
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
			groupBy: 'none',
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
			groupBy: 'none',
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

describe('narrow views', () => {
	it('names every cell by its column, so a narrow view can drop the trailing ones and keep Name', async () => {
		const { client } = clientWith(3);
		renderList(client);
		const list = await screen.findByRole('listbox');
		const row = list.querySelector('[role=option]')!;
		expect(
			[...row.querySelectorAll('[data-column]')].map((cell) => cell.getAttribute('data-column')),
		).toEqual(['size', 'modified', 'kind']);
		expect(
			[...screen.getAllByRole('button', { name: /^(Name|Size|Modified|Kind)/ })].map((button) =>
				button.getAttribute('data-column'),
			),
		).toEqual(['name', 'size', 'modified', 'kind']);
	});

	it('keeps Name a readable minimum and drops Kind, Modified and Size as the view narrows', () => {
		// jsdom lays nothing out, so the stylesheet's rules are checked as written.
		const css = readFileSync(resolve(process.cwd(), 'src/browse/ListView.module.css'), 'utf8');
		expect(css).toContain('container-type: inline-size');
		expect(css).toMatch(/--wp-name-min:\s*120px/);
		expect(css).toMatch(
			/grid-template-columns:\s*minmax\(var\(--wp-name-min\), 1fr\) 88px 168px 96px/,
		);
		for (const [width, column] of [
			['571', 'kind'],
			['463', 'modified'],
			['283', 'size'],
		] as const) {
			const rule = new RegExp(
				`@container list \\(max-width: ${width}px\\)[^]*?\\[data-column='${column}'\\]\\s*\\{\\s*display: none`,
			);
			expect(css, `${column} hides at ${width}px`).toMatch(rule);
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

describe('selection and keyboard', () => {
	async function ready(count = 100, onOpen?: (entry: Entry) => void) {
		const { client } = clientWith(count);
		renderList(client, onOpen);
		const list = await screen.findByRole('listbox');
		await waitFor(() => expect(rows()[0]).not.toHaveAttribute('data-placeholder'));
		return { client, list };
	}
	const key = (list: HTMLElement, name: string, init: KeyboardEventInit = {}) =>
		fireEvent.keyDown(list, { key: name, ...init });
	const selectedRows = () => rows().filter((row) => row.getAttribute('aria-selected') === 'true');
	const activeRow = () => rows().find((row) => row.hasAttribute('data-active'));
	// The row the keyboard is on, whether or not it is drawn (the virtualiser scrolls in a browser,
	// but happy-dom does no layout).
	const activePosition = (list: HTMLElement) =>
		Number(list.getAttribute('aria-activedescendant')?.split('-row-')[1]) + 1;

	it('puts the focus on the first row when the list takes focus, without selecting', async () => {
		const { list } = await ready();
		act(() => list.focus());
		expect(list).toHaveAttribute('aria-activedescendant', rows()[0]!.id);
		expect(activeRow()).toBe(rows()[0]);
		expect(selectedRows()).toHaveLength(0);
	});

	it('moves with the arrow keys, selecting what it lands on', async () => {
		const { list } = await ready();
		act(() => list.focus());
		key(list, 'ArrowDown');
		await waitFor(() => expect(selectedRows()).toHaveLength(1));
		expect(selectedRows()[0]).toHaveAttribute('aria-posinset', '2');
		expect(list).toHaveAttribute('aria-activedescendant', selectedRows()[0]!.id);
		key(list, 'ArrowUp');
		await waitFor(() => expect(selectedRows()[0]).toHaveAttribute('aria-posinset', '1'));
		key(list, 'ArrowUp');
		expect(activeRow()).toHaveAttribute('aria-posinset', '1');
	});

	it('lets a space continue a type-ahead prefix, and keeps Space from scrolling otherwise', async () => {
		const client = new FakeVfsClient();
		client.setFolder(
			FOLDER,
			['alpha', 'my file.txt', 'my other.txt', 'zebra'].map((name, i) => makeEntry(i + 1, name)),
		);
		renderList(client);
		const list = await screen.findByRole('listbox');
		await waitFor(() => expect(rows()[0]).not.toHaveAttribute('data-placeholder'));
		act(() => list.focus());
		// With nothing typed, Space does nothing but no longer scrolls the list.
		expect(fireEvent.keyDown(list, { key: ' ' })).toBe(false);
		for (const character of ['m', 'y', ' ', 'o']) key(list, character);
		await waitFor(() => expect(selectedRows()).toHaveLength(1));
		expect(selectedRows()[0]).toHaveTextContent('my other.txt');
	});

	it('abandons a pending type-ahead scan when another navigation key is pressed', async () => {
		const client = new FakeVfsClient();
		const names = Array.from({ length: 700 }, (_, i) => `a-${String(i).padStart(4, '0')}`);
		names[699] = 'z-last';
		client.setFolder(
			FOLDER,
			names.map((name, i) => makeEntry(i + 1, name)),
		);
		let armed = false;
		let release: () => void = () => {};
		const gate = new Promise<void>((resolve) => {
			release = resolve;
		});
		const slow = new Proxy(client, {
			get(target, property) {
				const value = Reflect.get(target, property, target) as unknown;
				if (property === 'getRange') {
					return async (...args: Parameters<VfsClient['getRange']>) => {
						if (armed) await gate;
						return target.getRange(...args);
					};
				}
				return typeof value === 'function' ? value.bind(target) : value;
			},
		});
		renderList(slow);
		const list = await screen.findByRole('listbox');
		await waitFor(() => expect(rows()[0]).not.toHaveAttribute('data-placeholder'));
		act(() => list.focus());
		armed = true;
		key(list, 'z');
		key(list, 'ArrowDown');
		await waitFor(() => expect(selectedRows()).toHaveLength(1));
		await act(async () => {
			release();
			await new Promise((resolve) => setTimeout(resolve, 20));
		});
		expect(selectedRows()).toHaveLength(1);
		expect(selectedRows()[0]).toHaveAttribute('aria-posinset', '2');
		expect(activePosition(list)).toBe(2);
	});

	it('jumps with Home, End and the page keys', async () => {
		const { list } = await ready();
		act(() => list.focus());
		key(list, 'End');
		await waitFor(() => expect(activePosition(list)).toBe(100));
		key(list, 'Home');
		await waitFor(() => expect(activePosition(list)).toBe(1));
		// A page is the ten visible rows less one.
		key(list, 'PageDown');
		await waitFor(() => expect(activePosition(list)).toBe(10));
		key(list, 'PageUp');
		await waitFor(() => expect(activePosition(list)).toBe(1));
	});

	it('extends the selection with Shift and the arrows, and announces the count', async () => {
		const { list } = await ready();
		act(() => list.focus());
		key(list, 'ArrowDown');
		await waitFor(() => expect(selectedRows()).toHaveLength(1));
		key(list, 'ArrowDown', { shiftKey: true });
		key(list, 'ArrowDown', { shiftKey: true });
		await waitFor(() => expect(selectedRows()).toHaveLength(3));
		expect(screen.getByText('3 items selected')).toHaveAttribute('aria-live', 'polite');
		key(list, 'ArrowUp', { shiftKey: true });
		await waitFor(() => expect(selectedRows()).toHaveLength(2));
		expect(screen.getByText('2 items selected')).toBeInTheDocument();
	});

	it('moves the focus alone with Ctrl and the arrows, and toggles with Ctrl+Space', async () => {
		const { list } = await ready();
		act(() => list.focus());
		key(list, 'ArrowDown', { ctrlKey: true });
		key(list, 'ArrowDown', { ctrlKey: true });
		expect(activeRow()).toHaveAttribute('aria-posinset', '3');
		expect(selectedRows()).toHaveLength(0);
		key(list, ' ', { ctrlKey: true });
		await waitFor(() => expect(selectedRows()).toHaveLength(1));
		expect(selectedRows()[0]).toHaveAttribute('aria-posinset', '3');
	});

	it('selects all, inverts and deselects from the keyboard', async () => {
		const { list } = await ready(100);
		act(() => list.focus());
		key(list, 'a', { ctrlKey: true });
		expect(screen.getByText('100 items selected')).toBeInTheDocument();
		expect(selectedRows().length).toBe(rows().length);
		key(list, 'i', { ctrlKey: true });
		expect(screen.getByText('No items selected')).toBeInTheDocument();
		expect(selectedRows()).toHaveLength(0);
		key(list, 'i', { ctrlKey: true });
		expect(screen.getByText('100 items selected')).toBeInTheDocument();
		key(list, 'Escape');
		expect(screen.getByText('No items selected')).toBeInTheDocument();
	});

	it('clears the selection with a plain click on empty space, but not with Shift or Ctrl held', async () => {
		const { list } = await ready(3);
		act(() => list.focus());
		key(list, 'a', { ctrlKey: true });
		expect(selectedRows()).toHaveLength(3);
		const scroller = list.parentElement!;
		// Shift and Ctrl clicks extend a selection; a slip on empty space must not throw it away.
		fireEvent.click(scroller, { shiftKey: true });
		fireEvent.click(scroller, { ctrlKey: true });
		expect(selectedRows()).toHaveLength(3);
		// A click on a row is the row's, not the background's.
		fireEvent.click(rows()[0]!, { shiftKey: true });
		expect(selectedRows().length).toBeGreaterThan(0);
		fireEvent.click(scroller);
		expect(selectedRows()).toHaveLength(0);
		expect(screen.getByText('No items selected')).toBeInTheDocument();
	});

	it('announces nothing until the person acts on the selection', async () => {
		await ready();
		expect(screen.queryByText(/selected/)).toBeNull();
	});

	it('announces a single item in the singular', async () => {
		const { list } = await ready();
		fireEvent.click(rows()[1]!);
		expect(screen.getByText('1 item selected')).toBeInTheDocument();
		expect(list).toHaveAttribute('aria-multiselectable', 'true');
	});

	it('replaces the selection on click, toggles on Ctrl-click and selects a range on Shift-click', async () => {
		await ready();
		fireEvent.click(rows()[1]!);
		expect(selectedRows()).toEqual([rows()[1]]);
		fireEvent.click(rows()[3]!, { ctrlKey: true });
		expect(selectedRows()).toEqual([rows()[1], rows()[3]]);
		fireEvent.click(rows()[1]!, { ctrlKey: true });
		expect(selectedRows()).toEqual([rows()[3]]);
		fireEvent.click(rows()[6]!, { shiftKey: true });
		await waitFor(() => expect(selectedRows()).toEqual(rows().slice(1, 7)));
		fireEvent.click(rows()[0]!);
		expect(selectedRows()).toEqual([rows()[0]]);
	});

	it('opens the focused entry on Enter and any entry on double-click', async () => {
		const onOpen = vi.fn();
		const { list } = await ready(100, onOpen);
		act(() => list.focus());
		key(list, 'ArrowDown');
		await waitFor(() => expect(selectedRows()).toHaveLength(1));
		key(list, 'Enter');
		expect(onOpen).toHaveBeenCalledTimes(1);
		expect(onOpen.mock.calls[0]![0]).toMatchObject({ name: expect.any(String) });
		fireEvent.doubleClick(rows()[4]!);
		expect(onOpen).toHaveBeenCalledTimes(2);
		expect(rows()[4]).toHaveTextContent(onOpen.mock.calls[1]![0].name);
	});

	it('jumps to a name by typing, including one on a page that is not loaded', async () => {
		const client = new FakeVfsClient();
		const names = Array.from(
			{ length: 900 },
			(_, i) => `${String.fromCharCode(97 + Math.floor(i / 100))}-${i}.txt`,
		);
		client.setFolder(
			FOLDER,
			names.map((name, i) => makeEntry(i, name)),
		);
		renderList(client);
		const list = await screen.findByRole('listbox');
		await waitFor(() => expect(rows()[0]).not.toHaveAttribute('data-placeholder'));
		act(() => list.focus());
		// "i" names start at view position 800, several pages from what is loaded.
		key(list, 'i');
		await waitFor(() => expect(activePosition(list)).toBe(801));
		await waitFor(() => expect(screen.getByText('1 item selected')).toBeInTheDocument());
	});

	it('leaves modified keys to the browser', async () => {
		const { list } = await ready();
		act(() => list.focus());
		const notPrevented = fireEvent.keyDown(list, { key: 'r', ctrlKey: true });
		expect(notPrevented).toBe(true);
	});
});

describe('live updates', () => {
	async function scrolledTo(position: number) {
		const { client } = clientWith(2000);
		renderList(client);
		const list = await screen.findByRole('listbox');
		const scroller = list.parentElement!;
		act(() => {
			scroller.scrollTop = position * 28;
			fireEvent.scroll(scroller);
		});
		const top = await waitFor(() => {
			const row = rows().find((r) => r.getAttribute('aria-posinset') === String(position + 1));
			expect(row).toBeDefined();
			expect(row).not.toHaveAttribute('data-placeholder');
			return row!;
		});
		return { client, list, scroller, name: top.textContent! };
	}
	const folderNamed = (id: number, name: string) => makeEntry(id, name, { kind: 'directory' });

	it('keeps the top visible entry in place when entries are inserted above it', async () => {
		const { client, scroller, name } = await scrolledTo(60);
		act(() =>
			client.addEntries(FOLDER, [
				folderNamed(900_001, '0-new-a'),
				folderNamed(900_002, '0-new-b'),
				folderNamed(900_003, '0-new-c'),
			]),
		);
		await waitFor(() => expect(scroller.scrollTop).toBe(63 * 28));
		await waitFor(() => {
			const row = rows().find((r) => r.getAttribute('aria-posinset') === '64');
			expect(row).toHaveTextContent(name);
			expect(row).not.toHaveAttribute('data-placeholder');
		});
	});

	it('keeps the top visible entry in place when entries above it are removed', async () => {
		const { client, scroller, name } = await scrolledTo(60);
		const probe = await client.openListing(FOLDER);
		const firstThree = (await client.getRange(probe.handle, 0, 3)).map((entry) => entry.id);
		act(() => client.removeEntries(FOLDER, firstThree));
		await waitFor(() => expect(scroller.scrollTop).toBe(57 * 28));
		await waitFor(() => {
			const row = rows().find((r) => r.getAttribute('aria-posinset') === '58');
			expect(row).toHaveTextContent(name);
		});
	});

	it('leaves the scroll position alone for changes below the viewport', async () => {
		const { client, scroller } = await scrolledTo(60);
		act(() => client.addEntries(FOLDER, [makeEntry(900_010, 'zzz-last.txt')]));
		await waitFor(() =>
			expect(screen.getByRole('listbox')).toHaveAttribute('aria-rowcount', '2001'),
		);
		expect(scroller.scrollTop).toBe(60 * 28);
	});

	it('stays at the top when scrolled to the top, so new entries are seen', async () => {
		const { client, list } = await ready2();
		const scroller = list.parentElement!;
		act(() => client.addEntries(FOLDER, [folderNamed(900_020, '0-new-top')]));
		await waitFor(() => expect(list).toHaveAttribute('aria-rowcount', '201'));
		expect(scroller.scrollTop).toBe(0);
		await waitFor(() => expect(rows()[0]).toHaveTextContent('0-new-top'));
	});

	it('never shows a blank row for entries that were already loaded', async () => {
		const { client, list } = await ready2();
		const before = rows().map((row) => row.textContent);
		act(() => client.updateEntries(FOLDER, new Map([[17, { modifiedMs: 1 }]])));
		expect(rows().map((row) => row.getAttribute('data-placeholder'))).toEqual(
			before.map(() => null),
		);
		expect(list).toHaveAttribute('aria-rowcount', '200');
	});

	it('keeps the selection on the same entries when others are inserted above', async () => {
		const { client } = await ready2();
		fireEvent.click(rows()[2]!);
		const name = rows()[2]!.textContent!;
		act(() => client.addEntries(FOLDER, [folderNamed(900_030, '0-new-top')]));
		await waitFor(() => {
			const selected = rows().filter((row) => row.getAttribute('aria-selected') === 'true');
			expect(selected).toHaveLength(1);
			expect(selected[0]).toHaveTextContent(name);
			expect(selected[0]).toHaveAttribute('aria-posinset', '4');
		});
	});

	async function ready2() {
		const { client } = clientWith(200);
		renderList(client);
		const list = await screen.findByRole('listbox');
		await waitFor(() => expect(rows()[0]).not.toHaveAttribute('data-placeholder'));
		return { client, list };
	}
});
