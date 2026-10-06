// Verifies that each folder remembers its own view, sort and grouping over a whole workspace: applied on opening, written on change, off by the setting, and reset
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { noWidths } from '../browse/columnWidths';
import { createFakeFolderViewsClient, NOTHING_CHOSEN } from '../services/fakeFolderViewsClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { DOCS, HOME, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(400, 600);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

const switcher = () => screen.getByRole('group', { name: 'View' });
const listButton = () => within(switcher()).getByRole('button', { name: 'List' });
const gridButton = () => within(switcher()).getByRole('button', { name: 'Grid' });
const option = (name: string) => screen.findByRole('option', { name: new RegExp(`^${name}`) });
const names = () => screen.getAllByRole('option').map((o) => o.textContent);
const emptySpace = () => screen.getByRole('listbox').parentElement!;

/** Two folders with files of different sizes, so a sort shows in the order of the rows. */
function twoFolders() {
	const client = new FakeVfsClient();
	client.setFolder({ display: '/', uri: 'file:///' }, []);
	client.setFolder(HOME, [
		makeEntry(1, 'docs', { kind: 'directory' }),
		makeEntry(2, 'a.txt', { size: 30 }),
		makeEntry(3, 'b.txt', { size: 10 }),
		makeEntry(4, 'c.txt', { size: 20 }),
	]);
	client.setFolder(DOCS, [
		makeEntry(1, 'x.pdf', { size: 5 }),
		makeEntry(2, 'y.pdf', { size: 50 }),
		makeEntry(3, 'z.pdf', { size: 25 }),
	]);
	return client;
}

async function go(tabs: FakeTabsApi, location: typeof HOME) {
	const active = (await tabs.getSnapshot()).active!;
	await act(async () => {
		await tabs.navigate(active, location);
	});
}

const bySizeDescending = {
	key: 'size',
	descending: true,
	directoriesFirst: true,
	groupBy: 'none',
} as const;

describe('a folder that remembers its own view', () => {
	it('opens with its remembered view and sort, and the next folder shows the window’s', async () => {
		const folderViews = createFakeFolderViewsClient({
			[DOCS.uri]: { mode: 'grid', sort: bySizeDescending },
		});
		const tabs = new FakeTabsApi();
		await renderWorkspace(twoFolders(), tabs, undefined, { folderViews });
		await option('docs');
		expect(listButton()).toHaveAttribute('aria-pressed', 'true');

		await go(tabs, DOCS);
		await option('y');
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
		expect(names()[0]).toMatch(/^y/);
		expect(names().map((n) => n?.[0])).toEqual(['y', 'z', 'x']);

		await go(tabs, HOME);
		await option('docs');
		await waitFor(() => expect(listButton()).toHaveAttribute('aria-pressed', 'true'));
		expect(names().map((n) => n?.[0])).toEqual(['d', 'a', 'b', 'c']);
	});

	it('applies a folder that is remembered when the first window loads, once the remembered views arrive', async () => {
		const folderViews = createFakeFolderViewsClient({
			[HOME.uri]: { mode: 'grid', sort: null },
		});
		const slow = folderViews.holdSnapshot();
		await renderWorkspace(twoFolders(), undefined, undefined, { folderViews });
		await option('docs');
		expect(listButton()).toHaveAttribute('aria-pressed', 'true');
		slow.release();
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
	});

	it('remembers the view chosen in a folder, for that folder only', async () => {
		const folderViews = createFakeFolderViewsClient();
		const tabs = new FakeTabsApi();
		await renderWorkspace(twoFolders(), tabs, undefined, { folderViews });
		await go(tabs, DOCS);
		await option('y');
		fireEvent.click(gridButton());
		await waitFor(() =>
			expect(folderViews.remembered).toEqual([
				{ key: DOCS.uri, patch: { ...NOTHING_CHOSEN, mode: 'grid' } },
			]),
		);

		await go(tabs, HOME);
		await option('docs');
		await waitFor(() => expect(listButton()).toHaveAttribute('aria-pressed', 'true'));
		expect(folderViews.view(HOME.uri)).toBeUndefined();

		await go(tabs, DOCS);
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
	});

	it('remembers the sort and the grouping chosen in a folder, and not for the window', async () => {
		const folderViews = createFakeFolderViewsClient();
		const tabs = new FakeTabsApi();
		await renderWorkspace(twoFolders(), tabs, undefined, { folderViews });
		await option('docs');
		fireEvent.contextMenu(emptySpace());
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Sort by' }));
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: /^Size/ }));
		await waitFor(() => expect(folderViews.view(HOME.uri)?.sort?.key).toBe('size'));
		expect(folderViews.view(HOME.uri)?.mode).toBeNull();

		fireEvent.contextMenu(emptySpace());
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Group by' }));
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: /^Group by Kind/ }));
		await waitFor(() => expect(folderViews.view(HOME.uri)?.sort?.groupBy).toBe('kind'));

		await go(tabs, DOCS);
		await option('x');
		expect(names().map((n) => n?.[0])).toEqual(['x', 'y', 'z']);
		expect(folderViews.view(DOCS.uri)).toBeUndefined();
	});

	it('follows a change another window makes to the folder on screen', async () => {
		const folderViews = createFakeFolderViewsClient();
		await renderWorkspace(twoFolders(), undefined, undefined, { folderViews });
		await option('docs');
		await act(async () => {
			folderViews.change(HOME.uri, { mode: 'grid', sort: bySizeDescending });
		});
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
		await waitFor(() => expect(names().map((n) => n?.[0])).toEqual(['d', 'a', 'c', 'b']));
		// What the window did to follow is not written back.
		expect(folderViews.remembered).toEqual([]);
	});

	it('is reset from the empty-space menu: the folder shows the window’s view again', async () => {
		const folderViews = createFakeFolderViewsClient({
			[HOME.uri]: { mode: 'grid', sort: bySizeDescending },
		});
		await renderWorkspace(twoFolders(), undefined, undefined, { folderViews });
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
		await option('a');
		fireEvent.contextMenu(emptySpace());
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Reset This Folder’s View' }));
		await waitFor(() => expect(folderViews.resets).toEqual([HOME.uri]));
		await waitFor(() => expect(listButton()).toHaveAttribute('aria-pressed', 'true'));
		await waitFor(() => expect(names().map((n) => n?.[0])).toEqual(['d', 'a', 'b', 'c']));
		expect(folderViews.view(HOME.uri)).toBeUndefined();
		// Nothing was written while the window followed the reset.
		expect(folderViews.remembered).toEqual([]);
		// With nothing remembered, the item is listed but disabled.
		fireEvent.contextMenu(emptySpace());
		expect(
			await screen.findByRole('menuitem', { name: 'Reset This Folder’s View' }),
		).toHaveAttribute('aria-disabled', 'true');
	});
});

describe('hidden files and the icon size', () => {
	it('are remembered by the folder they were chosen in, and the next folder keeps the window’s', async () => {
		const folderViews = createFakeFolderViewsClient();
		const tabs = new FakeTabsApi();
		const client = twoFolders();
		client.setFolder(DOCS, [makeEntry(1, 'x.pdf'), makeEntry(2, '.secret')]);
		await renderWorkspace(client, tabs, undefined, { folderViews });
		await go(tabs, DOCS);
		await option('x');
		expect(screen.queryByRole('option', { name: /^\.secret/ })).toBeNull();
		fireEvent.keyDown(window, { key: 'h', ctrlKey: true });
		await option('\\.secret');
		await waitFor(() => expect(folderViews.view(DOCS.uri)?.showHidden).toBe(true));

		await go(tabs, HOME);
		await option('docs');
		await waitFor(() => expect(screen.queryByRole('option', { name: /^\.secret/ })).toBeNull());
		expect(folderViews.view(HOME.uri)).toBeUndefined();

		await go(tabs, DOCS);
		await option('\\.secret');
	});

	it('come back with the folder, and Reset This Folder’s View puts them all back', async () => {
		const folderViews = createFakeFolderViewsClient({
			[HOME.uri]: { mode: 'grid', showHidden: true, iconSize: 200 },
		});
		await renderWorkspace(twoFolders(), undefined, undefined, { folderViews });
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
		await waitFor(() =>
			expect(screen.getByRole('listbox').style.getPropertyValue('--wp-grid-size')).toBe('200px'),
		);
		fireEvent.contextMenu(emptySpace());
		const hidden = await screen.findByRole('menuitemcheckbox', { name: /^Show hidden files/ });
		expect(hidden).toHaveAttribute('aria-checked', 'true');
		fireEvent.keyDown(hidden, { key: 'Escape' });
		fireEvent.contextMenu(emptySpace());
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Reset This Folder’s View' }));
		await waitFor(() => expect(listButton()).toHaveAttribute('aria-pressed', 'true'));
		fireEvent.click(gridButton());
		await waitFor(() =>
			expect(screen.getByRole('listbox').style.getPropertyValue('--wp-grid-size')).toBe('96px'),
		);
		expect(folderViews.view(HOME.uri)?.iconSize).not.toBe(200);
	});
});

describe('list column widths', () => {
	const widths = { ...noWidths(), size: 150 };
	const sizeVariable = () =>
		screen
			.getByRole('listbox')
			.closest<HTMLElement>('[data-layout]')!
			.style.getPropertyValue('--wp-col-size');

	it('follow the folder shown, and the next folder keeps the columns’ own widths', async () => {
		const folderViews = createFakeFolderViewsClient({ [DOCS.uri]: { columnWidths: widths } });
		const tabs = new FakeTabsApi();
		await renderWorkspace(twoFolders(), tabs, undefined, { folderViews });
		await option('docs');
		expect(sizeVariable()).toBe('');

		await go(tabs, DOCS);
		await option('y');
		await waitFor(() => expect(sizeVariable()).toBe('150px'));

		await go(tabs, HOME);
		await option('docs');
		await waitFor(() => expect(sizeVariable()).toBe(''));
		expect(folderViews.remembered).toEqual([]);
	});

	it('are forgotten with the rest of the folder’s view by Reset This Folder’s View', async () => {
		const folderViews = createFakeFolderViewsClient({
			[HOME.uri]: { columnWidths: widths },
		});
		await renderWorkspace(twoFolders(), undefined, undefined, { folderViews });
		await waitFor(() => expect(sizeVariable()).toBe('150px'));
		fireEvent.contextMenu(emptySpace());
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Reset This Folder’s View' }));
		await waitFor(() => expect(sizeVariable()).toBe(''));
		expect(folderViews.view(HOME.uri)).toBeUndefined();
	});
});

describe('with remembering turned off', () => {
	const off = () =>
		createFakeSettingsClient({
			...DEFAULT_SETTINGS,
			general: { ...DEFAULT_SETTINGS.general, rememberFolderViews: false },
		});

	it('shows every folder the window’s view, writes nothing, and offers no reset', async () => {
		const folderViews = createFakeFolderViewsClient({
			[DOCS.uri]: { mode: 'grid', sort: bySizeDescending },
		});
		const tabs = new FakeTabsApi();
		await renderWorkspace(twoFolders(), tabs, undefined, {
			folderViews,
			settings: off(),
		});
		await option('docs');
		await go(tabs, DOCS);
		await option('x');
		expect(listButton()).toHaveAttribute('aria-pressed', 'true');
		expect(names().map((n) => n?.[0])).toEqual(['x', 'y', 'z']);

		// The choice is the window's again, as it was before folders remembered.
		fireEvent.click(gridButton());
		await go(tabs, HOME);
		await option('docs');
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
		fireEvent.contextMenu(emptySpace());
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Sort by' }));
		fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: /^Size/ }));
		await waitFor(() => expect(names().map((n) => n?.[0])).toEqual(['d', 'b', 'c', 'a']));
		await go(tabs, DOCS);
		await option('x');
		expect(names().map((n) => n?.[0])).toEqual(['x', 'z', 'y']);

		expect(folderViews.remembered).toEqual([]);
		fireEvent.contextMenu(emptySpace());
		await screen.findByRole('menu', { name: 'Folder actions' });
		expect(screen.queryByRole('menuitem', { name: 'Reset This Folder’s View' })).toBeNull();
	});

	it('brings the remembered views back when it is turned on again, and puts the window’s back when it is turned off', async () => {
		const folderViews = createFakeFolderViewsClient({
			[HOME.uri]: { mode: 'grid', sort: bySizeDescending },
		});
		const settings = createFakeSettingsClient();
		await renderWorkspace(twoFolders(), undefined, undefined, { folderViews, settings });
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));

		await act(async () => {
			settings.change({
				...DEFAULT_SETTINGS,
				general: { ...DEFAULT_SETTINGS.general, rememberFolderViews: false },
			});
		});
		await waitFor(() => expect(listButton()).toHaveAttribute('aria-pressed', 'true'));
		await waitFor(() => expect(names().map((n) => n?.[0])).toEqual(['d', 'a', 'b', 'c']));

		await act(async () => {
			settings.change(DEFAULT_SETTINGS);
		});
		await waitFor(() => expect(gridButton()).toHaveAttribute('aria-pressed', 'true'));
		await waitFor(() => expect(names().map((n) => n?.[0])).toEqual(['d', 'a', 'c', 'b']));
		expect(folderViews.remembered).toEqual([]);
	});
});

describe('where a folder cannot remember', () => {
	it('keeps the Trash’s and Overview’s view to themselves', async () => {
		const folderViews = createFakeFolderViewsClient();
		const tabs = new FakeTabsApi();
		await renderWorkspace(twoFolders(), tabs, undefined, { folderViews });
		await option('docs');
		await go(tabs, { display: 'Overview', uri: 'overview:/' });
		await waitFor(() => expect(gridButton()).toBeInTheDocument());
		fireEvent.click(gridButton());
		await go(tabs, HOME);
		await option('docs');
		expect(folderViews.remembered).toEqual([]);
		// The window’s mode went back to the folder’s, which remembers nothing.
		await waitFor(() => expect(listButton()).toHaveAttribute('aria-pressed', 'true'));
	});
});
