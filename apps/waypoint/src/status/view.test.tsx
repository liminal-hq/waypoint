// Verifies the view switcher in the footer and the empty-space menu over the whole browsing area
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { HOME, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(400, 600);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

const option = (name: string) => screen.findByRole('option', { name: new RegExp(`^${name}`) });
const switcher = () => screen.getByRole('group', { name: 'View' });
const listButton = () => within(switcher()).getByRole('button', { name: 'List' });
const gridButton = () => within(switcher()).getByRole('button', { name: 'Grid' });
const names = () => screen.getAllByRole('option').map((o) => o.textContent);

function folderWithHidden() {
	const client = new FakeVfsClient();
	client.setFolder({ display: '/', uri: 'file:///' }, []);
	client.setFolder(HOME, [
		makeEntry(1, 'beta.txt', { size: 10 }),
		makeEntry(2, 'alpha.txt', { size: 30 }),
		makeEntry(3, '.hidden', { size: 20 }),
		makeEntry(4, 'gamma.txt', { size: 5 }),
	]);
	return client;
}

describe('the view switcher', () => {
	it('sits in the status bar, starts on List and shows which view is chosen', async () => {
		await renderWorkspace();
		await option('docs');
		const bar = screen.getByRole('group', { name: 'Status bar' });
		expect(bar).toContainElement(switcher());
		expect(listButton()).toHaveAttribute('aria-pressed', 'true');
		expect(gridButton()).toHaveAttribute('aria-pressed', 'false');
		expect(screen.getByRole('group', { name: 'Sort the list' })).toBeInTheDocument();
	});

	it('swaps the list for the grid, keeping the selection and the listing', async () => {
		const { client } = await renderWorkspace();
		fireEvent.click(await option('notes.txt'));
		fireEvent.click(gridButton());
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
		// The grid has no column header, and the same entry is still selected.
		expect(screen.queryByRole('group', { name: 'Sort the list' })).toBeNull();
		expect(await option('notes.txt')).toHaveAttribute('aria-selected', 'true');
		expect(client.openCount).toBe(1);
		fireEvent.click(listButton());
		expect(await screen.findByRole('group', { name: 'Sort the list' })).toBeInTheDocument();
		expect(await option('notes.txt')).toHaveAttribute('aria-selected', 'true');
	});

	it('shows the icon size slider only in the grid, in steps between 48 and 256', async () => {
		await renderWorkspace();
		await option('docs');
		expect(screen.queryByRole('slider')).toBeNull();
		fireEvent.click(gridButton());
		const slider = await screen.findByRole('slider', { name: 'Icon size' });
		expect(slider).toHaveAttribute('min', '48');
		expect(slider).toHaveAttribute('max', '256');
		expect(slider).toHaveAttribute('step', '8');
		expect(slider).toHaveValue('96');
		// The slider has a hover tooltip with the current size, as the buttons beside it have.
		expect(slider).toHaveAttribute('title', 'Icon size: 96 pixels');
		fireEvent.change(slider, { target: { value: '200' } });
		await waitFor(() =>
			expect(screen.getByRole('listbox').style.getPropertyValue('--wp-grid-size')).toBe('200px'),
		);
		expect(slider).toHaveAttribute('aria-valuetext', '200 pixels');
	});

	it('follows Ctrl+1 for Grid and Ctrl+2 for List', async () => {
		await renderWorkspace();
		await option('docs');
		fireEvent.keyDown(window, { key: '1', ctrlKey: true });
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
		fireEvent.keyDown(window, { key: '2', ctrlKey: true });
		await waitFor(() => expect(listButton()).toHaveAttribute('aria-pressed', 'true'));
	});

	it('keeps the chosen view when navigating to another folder', async () => {
		await renderWorkspace();
		fireEvent.click(gridButton());
		fireEvent.doubleClick(await option('docs'));
		await option('report.pdf');
		expect(screen.queryByRole('group', { name: 'Sort the list' })).toBeNull();
		expect(gridButton()).toHaveAttribute('aria-pressed', 'true');
	});

	it('navigates with the arrows in both dimensions once in the grid', async () => {
		await renderWorkspace();
		fireEvent.click(gridButton());
		await option('docs');
		const list = screen.getByRole('listbox');
		act(() => list.focus());
		fireEvent.keyDown(list, { key: 'ArrowRight' });
		await waitFor(() =>
			expect(screen.getAllByRole('option', { selected: true })[0]).toHaveTextContent('music'),
		);
	});
});

describe('the empty-space menu', () => {
	const emptySpace = () => screen.getByRole('listbox').parentElement!;

	it('offers sort and hidden files, with the current choices checked', async () => {
		await renderWorkspace(folderWithHidden());
		await option('beta');
		fireEvent.contextMenu(emptySpace());
		const menu = await screen.findByRole('menu', { name: 'Folder actions' });
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Sort by' }));
		const checked = (name: string) =>
			within(menu).getByRole('menuitemcheckbox', { name: new RegExp(`^${name}`) });
		expect(checked('Name')).toHaveAttribute('aria-checked', 'true');
		expect(checked('Size')).toHaveAttribute('aria-checked', 'false');
		expect(checked('Descending')).toHaveAttribute('aria-checked', 'false');
		expect(checked('Folders first')).toHaveAttribute('aria-checked', 'true');
		expect(checked('Show hidden files')).toHaveAttribute('aria-checked', 'false');
	});

	it('sorts the listing from the menu, and reverses it', async () => {
		await renderWorkspace(folderWithHidden());
		await option('alpha');
		expect(names().map((n) => n?.replace(/\.txt.*/, ''))).toEqual(['alpha', 'beta', 'gamma']);
		fireEvent.contextMenu(emptySpace());
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Sort by' }));
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: /^Size/ }));
		await waitFor(() => expect(names()[0]).toMatch(/^gamma/));
		fireEvent.contextMenu(emptySpace());
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Sort by' }));
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: /^Descending/ }));
		await waitFor(() => expect(names()[0]).toMatch(/^alpha/));
	});

	it('shows and hides hidden files in every view, and from Ctrl+H', async () => {
		await renderWorkspace(folderWithHidden());
		await option('alpha');
		expect(screen.getByRole('group', { name: 'Status bar' })).toHaveTextContent('3 items');
		fireEvent.contextMenu(emptySpace());
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: /^Show hidden files/ }));
		await waitFor(() =>
			expect(screen.getByRole('group', { name: 'Status bar' })).toHaveTextContent('4 items'),
		);
		expect(await option('\\.hidden')).toBeInTheDocument();

		fireEvent.click(gridButton());
		await option('\\.hidden');
		fireEvent.keyDown(window, { key: 'h', ctrlKey: true });
		await waitFor(() =>
			expect(screen.getByRole('group', { name: 'Status bar' })).toHaveTextContent('3 items'),
		);
		expect(screen.queryByRole('option', { name: /^\.hidden/ })).toBeNull();
	});

	it('is also the menu of an empty folder, and opens from the keyboard when nothing is focused', async () => {
		const client = folderWithHidden();
		client.setFolder(HOME, []);
		await renderWorkspace(client);
		const empty = await screen.findByText('This folder is empty.');
		fireEvent.contextMenu(empty);
		const menu = await screen.findByRole('menu', { name: 'Folder actions' });
		expect(
			within(menu).getByRole('menuitemcheckbox', { name: /^Show hidden files/ }),
		).toBeInTheDocument();
		fireEvent.keyDown(menu, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());
	});

	it('opens from the menu key when no entry has focus', async () => {
		await renderWorkspace(folderWithHidden());
		await option('alpha');
		fireEvent.click(gridButton());
		await option('alpha');
		const list = screen.getByRole('listbox');
		fireEvent.keyDown(list, { key: 'F10', shiftKey: true });
		expect(await screen.findByRole('menu')).toBeInTheDocument();
	});
});
