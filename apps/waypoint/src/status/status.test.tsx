// Verifies the status bar, Open, and the read-only context menu over the whole browsing area
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { formatSize } from '../browse/format';
import { stubLayout } from '../test/browseHarness';
import { createTree, DOCS, HOME, renderWorkspace } from '../test/workspaceHarness';
import { SUMMARY_DELAY_MS } from './useSelectionSummary';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

const option = (name: string) => screen.findByRole('option', { name: new RegExp(`^${name}`) });
const bar = () => screen.getByRole('group', { name: 'Status bar' });
const announcer = () => within(bar()).getByRole('status');

describe('the status bar', () => {
	it('shows the item count of the folder', async () => {
		await renderWorkspace();
		await option('docs');
		expect(bar()).toHaveTextContent('4 items');
	});

	it('shows the selection count at once and its size from Rust a moment later', async () => {
		await renderWorkspace();
		fireEvent.click(await option('notes.txt'));
		expect(bar()).toHaveTextContent('1 item selected');
		// Sizes are 1024 + id in the fake, so notes.txt (id 3) is 1027 bytes.
		await waitFor(() => expect(bar()).toHaveTextContent(`· ${formatSize(1027)}`));

		const list = screen.getByRole('listbox');
		fireEvent.keyDown(list, { key: 'a', ctrlKey: true });
		expect(bar()).toHaveTextContent('4 items selected');
		await waitFor(() => expect(bar()).toHaveTextContent(`· ${formatSize(1027 + 1028)}`));
	});

	it('asks Rust once for a selection that changes quickly', async () => {
		const client = createTree();
		const summarise = vi.spyOn(client, 'summariseSelection');
		await renderWorkspace(client);
		const list = await screen.findByRole('listbox');
		await option('docs');
		act(() => list.focus());
		for (let i = 0; i < 4; i++) fireEvent.keyDown(list, { key: 'ArrowDown' });
		await waitFor(() => expect(summarise).toHaveBeenCalled());
		await new Promise((resolve) => setTimeout(resolve, SUMMARY_DELAY_MS + 50));
		expect(summarise).toHaveBeenCalledTimes(1);
	});

	it('shows free space, and nothing where it is unknown', async () => {
		const client = createTree();
		client.setFreeSpace(HOME, { freeBytes: 5_000_000_000, totalBytes: 10_000_000_000 });
		client.setFreeSpace(DOCS, null);
		const view = await renderWorkspace(client);
		await waitFor(() => expect(bar()).toHaveTextContent(`${formatSize(5_000_000_000)} free`));
		fireEvent.doubleClick(await option('docs'));
		await option('report.pdf');
		await waitFor(() => expect(bar()).not.toHaveTextContent('free'));
		view.unmount();
	});

	it('announces the selection count in a polite live region, and nothing before the first action', async () => {
		await renderWorkspace();
		const list = await screen.findByRole('listbox');
		await option('docs');
		expect(announcer()).toHaveAttribute('aria-live', 'polite');
		expect(announcer()).toHaveTextContent('');
		fireEvent.keyDown(list, { key: 'a', ctrlKey: true });
		await waitFor(() => expect(announcer()).toHaveTextContent('4 items selected'));
		fireEvent.keyDown(list, { key: 'Escape' });
		await waitFor(() => expect(announcer()).toHaveTextContent('No items selected'));
		// The list does not announce a second time.
		expect(
			screen.getAllByRole('status').filter((el) => el.textContent?.includes('selected')),
		).toHaveLength(1);
	});

	it('leaves a slot for the view switcher on the right', async () => {
		await renderWorkspace();
		await option('docs');
		expect(bar()).toBeInTheDocument();
	});
});

describe('Open', () => {
	it('opens a folder in place on Enter and double-click', async () => {
		await renderWorkspace();
		const list = await screen.findByRole('listbox');
		await option('docs');
		fireEvent.click(await option('docs'));
		act(() => list.focus());
		fireEvent.keyDown(list, { key: 'Enter' });
		await option('report.pdf');
		fireEvent.click(screen.getByRole('button', { name: 'Back' }));
		fireEvent.doubleClick(await option('music'));
		await option('song.mp3');
	});

	it('opens a file in its default application through the client', async () => {
		const client = createTree();
		await renderWorkspace(client);
		const list = await screen.findByRole('listbox');
		fireEvent.doubleClick(await option('notes.txt'));
		await waitFor(() => expect(client.opened).toHaveLength(1));
		expect(client.opened[0]!.id).toBe(3);

		fireEvent.click(await option('photo.jpg'));
		act(() => list.focus());
		fireEvent.keyDown(list, { key: 'Enter' });
		await waitFor(() => expect(client.opened).toHaveLength(2));
		expect(client.opened[1]!.id).toBe(4);
	});

	it('says so in the status bar when a file will not open', async () => {
		const client = createTree();
		client.failOpeningEntries({ kind: 'permissionDenied', location: HOME });
		await renderWorkspace(client);
		fireEvent.doubleClick(await option('notes.txt'));
		expect(await within(bar()).findByRole('alert')).toHaveTextContent('Could not open notes.txt.');
	});
});

describe('failure notices', () => {
	it('says a copy failed, not that the entry could not be opened', async () => {
		const writeText = vi.fn().mockRejectedValue(new Error('denied'));
		Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		await renderWorkspace();
		fireEvent.contextMenu(await option('notes.txt'));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Copy Path' }));
		expect(await within(bar()).findByRole('alert')).toHaveTextContent(
			'Could not copy the path of notes.txt.',
		);
	});

	it('keeps a repeated notice for its full time', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		try {
			const client = createTree();
			client.failOpeningEntries({ kind: 'permissionDenied', location: HOME });
			vi.spyOn(console, 'warn').mockImplementation(() => {});
			await renderWorkspace(client);
			fireEvent.doubleClick(await option('notes.txt'));
			await within(bar()).findByRole('alert');
			await act(() => vi.advanceTimersByTimeAsync(5000));
			fireEvent.doubleClick(await option('notes.txt'));
			await act(() => vi.advanceTimersByTimeAsync(10));
			await act(() => vi.advanceTimersByTimeAsync(2000));
			expect(within(bar()).queryByRole('alert')).not.toBeNull();
			await act(() => vi.advanceTimersByTimeAsync(5000));
			expect(within(bar()).queryByRole('alert')).toBeNull();
		} finally {
			vi.useRealTimers();
		}
	});
});

describe('the context menu', () => {
	it('closes when the active tab changes while it is open', async () => {
		const h = await renderWorkspace();
		fireEvent.contextMenu(await option('notes.txt'));
		await screen.findByRole('menu');
		await act(async () => {
			await h.tabs.openTab(DOCS);
		});
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
	});

	it('offers Open and Copy Path on a file, selects it, and opens it', async () => {
		const client = createTree();
		await renderWorkspace(client);
		const row = await option('notes.txt');
		fireEvent.contextMenu(row);
		const menu = await screen.findByRole('menu', { name: 'Item actions' });
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((i) => i.textContent),
		).toEqual([expect.stringContaining('Open'), 'Copy Path']);
		expect(row).toHaveAttribute('aria-selected', 'true');
		fireEvent.click(within(menu).getByRole('menuitem', { name: /^Open/ }));
		await waitFor(() => expect(client.opened).toHaveLength(1));
		expect(screen.queryByRole('menu')).toBeNull();
	});

	it('adds Open in New Tab for a folder, which opens a background tab', async () => {
		await renderWorkspace();
		fireEvent.contextMenu(await option('docs'));
		const menu = await screen.findByRole('menu');
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Open in New Tab' }));
		await waitFor(() => expect(screen.getAllByRole('tab')).toHaveLength(2));
		expect(screen.getAllByRole('tab')[0]).toHaveAttribute('aria-selected', 'true');
	});

	it('copies the path as Rust displays it', async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
		await renderWorkspace();
		fireEvent.contextMenu(await option('notes.txt'));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Copy Path' }));
		await waitFor(() => expect(writeText).toHaveBeenCalledWith('/home/test/notes.txt'));
	});

	it('keeps a multiple selection when right-clicking inside it', async () => {
		await renderWorkspace();
		const list = await screen.findByRole('listbox');
		await option('docs');
		fireEvent.keyDown(list, { key: 'a', ctrlKey: true });
		fireEvent.contextMenu(await option('notes.txt'));
		await screen.findByRole('menu');
		expect(await option('photo.jpg')).toHaveAttribute('aria-selected', 'true');
	});

	it('opens from the menu key for the focused entry and Escape closes it', async () => {
		await renderWorkspace();
		const list = await screen.findByRole('listbox');
		await option('docs');
		act(() => list.focus());
		fireEvent.keyDown(list, { key: 'ArrowDown' });
		fireEvent.keyDown(list, { key: 'ContextMenu' });
		const menu = await screen.findByRole('menu', { name: 'Item actions' });
		expect(within(menu).getByRole('menuitem', { name: 'Open in New Tab' })).toBeInTheDocument();
		fireEvent.keyDown(menu, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
	});
});
