// Verifies the grid view: listbox and option roles, columns from the width and size, 2-D keyboard, selection and the cap
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeVfsClient } from '../services/fakeVfsClient';
import type { VfsClient } from '../services/vfsClient';
import { clientWith, FOLDER, hugeClient, stubLayout } from '../test/browseHarness';
import { GridView } from './GridView';
import { useListingSession, type ListingSession } from './useListingSession';
import { VfsClientProvider } from './VfsClientContext';
import { useVfsClient } from './VfsClientContext';
import type { MenuRequest } from './useListInteractions';

// 400 px tall and 600 px wide: at 96 px icons a cell is 120 by 160, so four columns and 2.5 rows.
let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(400, 600);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

interface Handlers {
	onOpen?: (entry: Entry) => void;
	onOpenInNewTab?: (entry: Entry) => void;
	onMenu?: (request: MenuRequest) => void;
}

function Host({ size, ...handlers }: { size: number } & Handlers) {
	const client = useVfsClient();
	const state = useListingSession(client, FOLDER);
	return <GridView state={state} size={size} {...handlers} />;
}

function renderGrid(client: VfsClient, size = 96, handlers: Handlers = {}) {
	return render(
		<VfsClientProvider client={client}>
			<Host size={size} {...handlers} />
		</VfsClientProvider>,
	);
}

const options = () => screen.queryAllByRole('option');
const list = () => screen.getByRole('listbox');

describe('the grid', () => {
	it('is a multi-selectable listbox of options, not a grid', async () => {
		const { client } = clientWith(1000);
		renderGrid(client);
		const box = await screen.findByRole('listbox', { name: 'Files' });
		expect(box).toHaveAttribute('aria-multiselectable', 'true');
		expect(screen.queryByRole('grid')).toBeNull();
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		const first = options()[0]!;
		expect(first).toHaveAttribute('aria-posinset', '1');
		expect(first).toHaveAttribute('aria-setsize', '1000');
		expect(first).toHaveAttribute('aria-selected', 'false');
	});

	it('lays out as many columns as fit, more for smaller icons, and renders only the viewport', async () => {
		const { client } = clientWith(1000);
		const view = renderGrid(client, 96);
		await screen.findByRole('listbox');
		expect(list().style.getPropertyValue('--wp-grid-columns')).toBe('4');
		expect(list().style.getPropertyValue('--wp-grid-size')).toBe('96px');
		// Virtualised: a few rows of four, not a thousand items.
		expect(options().length).toBeLessThan(60);
		view.rerender(
			<VfsClientProvider client={client}>
				<Host size={48} />
			</VfsClientProvider>,
		);
		await waitFor(() => expect(list().style.getPropertyValue('--wp-grid-columns')).toBe('8'));
		expect(list().style.getPropertyValue('--wp-grid-size')).toBe('48px');
	});

	it('shows names and fetches pages as it scrolls', async () => {
		const { client } = clientWith(1000);
		renderGrid(client);
		expect((await screen.findAllByText(/^folder-/)).length).toBeGreaterThan(0);
		const getRange = vi.spyOn(client, 'getRange');
		const scroller = list().parentElement!;
		scroller.scrollTop = 160 * 100;
		fireEvent.scroll(scroller);
		await waitFor(() => expect(getRange).toHaveBeenCalled());
	});

	it('shows the empty message for an empty folder', async () => {
		const client = new FakeVfsClient();
		client.setFolder(FOLDER, []);
		renderGrid(client);
		expect(await screen.findByText('This folder is empty.')).toBeInTheDocument();
	});
});

describe('keyboard', () => {
	async function focused(count = 30) {
		const { client } = clientWith(count);
		renderGrid(client);
		await screen.findByRole('listbox');
		await waitFor(() => expect(options()[0]).toHaveTextContent(/\w/));
		act(() => list().focus());
		return list();
	}
	// The 1-based position of the keyboard focus, read from `aria-activedescendant`.
	const at = () =>
		Number(/-item-(\d+)$/.exec(list().getAttribute('aria-activedescendant') ?? '')?.[1]) + 1;
	const selected = () => options().filter((o) => o.getAttribute('aria-selected') === 'true');

	it('moves Left to the next item in a right-to-left layout', async () => {
		const box = await focused();
		document.documentElement.style.direction = 'rtl';
		try {
			fireEvent.keyDown(box, { key: 'ArrowLeft' });
			await waitFor(() => expect(at()).toBe(2));
			fireEvent.keyDown(box, { key: 'ArrowRight' });
			await waitFor(() => expect(at()).toBe(1));
		} finally {
			document.documentElement.style.removeProperty('direction');
		}
	});

	it('moves one item sideways and one row vertically, selecting as it goes', async () => {
		const box = await focused();
		fireEvent.keyDown(box, { key: 'ArrowRight' });
		await waitFor(() => expect(at()).toBe(2));
		fireEvent.keyDown(box, { key: 'ArrowDown' });
		await waitFor(() => expect(at()).toBe(6));
		expect(selected()).toHaveLength(1);
		fireEvent.keyDown(box, { key: 'ArrowLeft' });
		await waitFor(() => expect(at()).toBe(5));
		fireEvent.keyDown(box, { key: 'ArrowUp' });
		await waitFor(() => expect(at()).toBe(1));
		// Up from the first row stays put.
		fireEvent.keyDown(box, { key: 'ArrowUp' });
		expect(at()).toBe(1);
	});

	it('goes to the ends with Home and End and by pages with Page Down and Page Up', async () => {
		const box = await focused(100);
		fireEvent.keyDown(box, { key: 'End' });
		await waitFor(() => expect(at()).toBe(100));
		fireEvent.keyDown(box, { key: 'Home' });
		await waitFor(() => expect(at()).toBe(1));
		fireEvent.keyDown(box, { key: 'PageDown' });
		// Two full rows of the 400 px viewport minus one is one row: position 1 + 4.
		await waitFor(() => expect(at()).toBe(5));
		fireEvent.keyDown(box, { key: 'PageUp' });
		await waitFor(() => expect(at()).toBe(1));
	});

	it('extends a range with Shift and selects all and inverts with Ctrl+A and Ctrl+I', async () => {
		const box = await focused();
		fireEvent.keyDown(box, { key: 'ArrowDown', shiftKey: true });
		await waitFor(() => expect(selected()).toHaveLength(5));
		fireEvent.keyDown(box, { key: 'a', ctrlKey: true });
		await waitFor(() =>
			expect(options().every((o) => o.getAttribute('aria-selected') === 'true')).toBe(true),
		);
		fireEvent.keyDown(box, { key: 'i', ctrlKey: true });
		await waitFor(() =>
			expect(options().some((o) => o.getAttribute('aria-selected') === 'true')).toBe(false),
		);
	});

	it('jumps to a match while typing', async () => {
		const box = await focused();
		fireEvent.keyDown(box, { key: 'f' });
		fireEvent.keyDown(box, { key: 'i' });
		fireEvent.keyDown(box, { key: 'l' });
		fireEvent.keyDown(box, { key: 'e' });
		await waitFor(() => expect(selected()[0]).toHaveTextContent(/^file-/));
	});
});

describe('selection and opening', () => {
	it('selects on click, toggles on Ctrl-click and ranges on Shift-click, like the list', async () => {
		const { client } = clientWith(30);
		renderGrid(client);
		await screen.findByRole('listbox');
		await waitFor(() => expect(options()[0]).toHaveTextContent(/\w/));
		fireEvent.click(options()[1]!);
		expect(options()[1]).toHaveAttribute('aria-selected', 'true');
		fireEvent.click(options()[3]!, { ctrlKey: true });
		expect(options()[1]).toHaveAttribute('aria-selected', 'true');
		expect(options()[3]).toHaveAttribute('aria-selected', 'true');
		fireEvent.click(options()[6]!, { shiftKey: true });
		await waitFor(() => expect(options()[5]).toHaveAttribute('aria-selected', 'true'));
		expect(options()[0]).toHaveAttribute('aria-selected', 'false');
	});

	it('opens on Enter and double-click, and a middle-click opens in a new tab', async () => {
		const { client } = clientWith(30);
		const onOpen = vi.fn();
		const onOpenInNewTab = vi.fn();
		renderGrid(client, 96, { onOpen, onOpenInNewTab });
		await screen.findByRole('listbox');
		await waitFor(() => expect(options()[0]).toHaveTextContent(/\w/));
		fireEvent.doubleClick(options()[2]!);
		expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ id: expect.any(Number) }), 1);
		fireEvent.click(options()[4]!);
		act(() => list().focus());
		fireEvent.keyDown(list(), { key: 'Enter' });
		expect(onOpen).toHaveBeenCalledTimes(2);
		fireEvent(
			options()[1]!,
			new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true }),
		);
		expect(onOpenInNewTab).toHaveBeenCalledTimes(1);
	});

	it('asks for an entry menu on right-click, and an empty-space menu elsewhere', async () => {
		const { client } = clientWith(30);
		const onMenu = vi.fn();
		renderGrid(client, 96, { onMenu });
		await screen.findByRole('listbox');
		await waitFor(() => expect(options()[0]).toHaveTextContent(/\w/));
		fireEvent.contextMenu(options()[2]!, { clientX: 10, clientY: 20 });
		expect(onMenu).toHaveBeenLastCalledWith(
			expect.objectContaining({ kind: 'entry', position: { x: 10, y: 20 }, keyboard: false }),
		);
		expect(options()[2]).toHaveAttribute('aria-selected', 'true');
		fireEvent.contextMenu(list().parentElement!, { clientX: 5, clientY: 6 });
		expect(onMenu).toHaveBeenLastCalledWith(expect.objectContaining({ kind: 'background' }));
	});

	it('opens the entry menu from the menu key, and the empty-space menu from Ctrl plus the menu key', async () => {
		const { client } = clientWith(30);
		const onMenu = vi.fn();
		renderGrid(client, 96, { onMenu });
		await screen.findByRole('listbox');
		await waitFor(() => expect(options()[0]).toHaveTextContent(/\w/));
		fireEvent.click(options()[1]!);
		fireEvent.keyDown(list(), { key: 'ContextMenu' });
		expect(onMenu).toHaveBeenLastCalledWith(
			expect.objectContaining({ kind: 'entry', keyboard: true }),
		);
		fireEvent.keyDown(list(), { key: 'ContextMenu', ctrlKey: true });
		expect(onMenu).toHaveBeenLastCalledWith(
			expect.objectContaining({ kind: 'background', keyboard: true }),
		);
	});

	it('leaves Alt+arrow keys to the window so history and up still work', async () => {
		const { client } = clientWith(30);
		renderGrid(client);
		await screen.findByRole('listbox');
		await waitFor(() => expect(options()[0]).toHaveTextContent(/\w/));
		for (const key of ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown']) {
			expect(fireEvent.keyDown(list(), { key, altKey: true })).toBe(true);
		}
	});

	it('keeps its own scroll offset apart from the list', async () => {
		const { client } = clientWith(1000);
		let captured: ListingSession | undefined;
		function Capture() {
			const c = useVfsClient();
			const state = useListingSession(c, FOLDER);
			if (state.status === 'ready') captured = state.session;
			return <GridView state={state} size={96} />;
		}
		const { container } = render(
			<VfsClientProvider client={client}>
				<Capture />
			</VfsClientProvider>,
		);
		await screen.findByRole('listbox');
		const scroller = list().parentElement!;
		scroller.scrollTop = 300;
		fireEvent.scroll(scroller);
		expect(captured?.view.gridScrollTop).toBe(300);
		expect(captured?.view.scrollTop).toBe(0);
		expect(container).toBeTruthy();
	});

	it('announces the selection itself unless the host does', async () => {
		const { client } = clientWith(30);
		renderGrid(client);
		await screen.findByRole('listbox');
		await waitFor(() => expect(options()[0]).toHaveTextContent(/\w/));
		fireEvent.click(options()[0]!);
		expect(await screen.findByText('1 item selected')).toBeInTheDocument();
	});
});

describe('the scroll cap', () => {
	it('shows the banner with the items left out, counted in rows of columns', async () => {
		renderGrid(hugeClient(5_000_000));
		const banner = await screen.findByText(/Showing the first/);
		// 33 554 428 / 160 px = 209 715 rows of four.
		expect(banner).toHaveTextContent('Showing the first 838,860 of 5,000,000 items');
		expect(banner).toHaveAttribute('data-notice', 'capped');
		const scroller = list().parentElement!;
		scroller.scrollTop = 33_000_000;
		fireEvent.scroll(scroller);
		await waitFor(() => {
			const last = options().at(-1)!;
			expect(Number(last.getAttribute('aria-posinset'))).toBeLessThanOrEqual(838_860);
		});
	});

	it('shows no banner when everything fits', async () => {
		const { client } = clientWith(500);
		renderGrid(client);
		await screen.findByRole('listbox');
		expect(screen.queryByText(/Showing the first/)).toBeNull();
	});
});

describe('entries without names yet', () => {
	it('draws a placeholder until the page arrives', async () => {
		const { client } = clientWith(600, 30);
		renderGrid(client);
		await screen.findByRole('listbox');
		expect(options()[0]).toHaveAttribute('data-placeholder');
		expect(options()[0]).toHaveAttribute('aria-busy', 'true');
		await waitFor(() => expect(options()[0]).not.toHaveAttribute('data-placeholder'));
	});
});
