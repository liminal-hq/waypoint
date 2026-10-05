// Verifies the sidebar in the Main window: sections, navigation, favourites editing and the Folders tree
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { fileLocation, makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, DOCS, HOME, MUSIC, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

const DOWNLOADS = fileLocation('/home/test/Downloads');
const PROJECTS = fileLocation('/srv/projects');
const GONE = fileLocation('/srv/gone');

function makePlaces(favourites = [{ label: 'Projects', location: PROJECTS }]) {
	return new FakePlacesClient({ places: fakePlaces('/home/test'), favourites });
}

async function setup(places = makePlaces()) {
	const client = createTree();
	client.setFolder(DOWNLOADS, [makeEntry(1, 'a.zip')]);
	client.setFolder(PROJECTS, [makeEntry(1, 'code.ts')]);
	client.setFolder(fileLocation('/srv'), [makeEntry(1, 'projects', { kind: 'directory' })]);
	const h = await renderWorkspace(client, undefined, places, { sidebar: true });
	await screen.findByRole('button', { name: 'Home' });
	return h;
}

const sidebar = () => screen.getByRole('navigation', { name: 'Sidebar' });
const placesGroup = () => within(sidebar()).getByRole('group', { name: 'Places' });
const favouritesGroup = () => within(sidebar()).getByRole('group', { name: 'Favourites' });
const showFolders = () => fireEvent.click(within(sidebar()).getByRole('tab', { name: 'Folders' }));
const tree = () => within(sidebar()).getByRole('tree', { name: 'Folders' });
const item = (name: string) => within(tree()).getByRole('treeitem', { name });
const activeLocation = async (h: Awaited<ReturnType<typeof setup>>) => {
	const snapshot = await h.tabs.getSnapshot();
	return snapshot.tabs.find((tab) => tab.id === snapshot.active)!.location;
};
const press = (element: Element | Window, key: string, init: KeyboardEventInit = {}) =>
	fireEvent.keyDown(element, { key, ...init });

describe('structure', () => {
	it('is a navigation landmark with a labelled group and heading for each section', async () => {
		await setup();
		expect(sidebar().tagName).toBe('NAV');
		for (const name of ['Places', 'Favourites']) {
			const group = within(sidebar()).getByRole('group', { name });
			expect(within(group).getByRole('heading', { name, level: 2 })).toBeInTheDocument();
		}
	});

	it('shows the places the plugin returned, in order, and marks the current folder', async () => {
		await setup();
		const names = within(placesGroup())
			.getAllByRole('button')
			.map((button) => button.textContent);
		expect(names.slice(1)).toEqual([
			'Overview',
			'Home',
			'Desktop',
			'Documents',
			'Downloads',
			'Pictures',
			'Music',
			'Videos',
			'Trash',
		]);
		expect(within(placesGroup()).getByRole('button', { name: 'Home' })).toHaveAttribute(
			'aria-current',
			'page',
		);
		expect(within(placesGroup()).getByRole('button', { name: 'Music' })).not.toHaveAttribute(
			'aria-current',
		);
	});

	it('follows the window: collapsing a section and hiding the sidebar with F9 are remembered', async () => {
		await setup();
		const heading = within(placesGroup()).getByRole('button', { name: 'Places' });
		expect(heading).toHaveAttribute('aria-expanded', 'true');
		fireEvent.click(heading);
		expect(within(placesGroup()).queryByRole('button', { name: 'Home' })).toBeNull();
		expect(heading).toHaveAttribute('aria-expanded', 'false');

		const toggle = screen.getByRole('button', { name: 'Sidebar' });
		expect(toggle).toHaveAttribute('aria-pressed', 'true');
		press(window, 'F9');
		expect(screen.queryByRole('navigation', { name: 'Sidebar' })).toBeNull();
		expect(toggle).toHaveAttribute('aria-pressed', 'false');
		fireEvent.click(toggle);
		expect(within(placesGroup()).queryByRole('button', { name: 'Home' })).toBeNull();
		fireEvent.click(within(placesGroup()).getByRole('button', { name: 'Places' }));
		expect(await within(placesGroup()).findByRole('button', { name: 'Home' })).toBeInTheDocument();
	});

	it('does not touch localStorage', async () => {
		const spy = vi.spyOn(Storage.prototype, 'setItem');
		await setup();
		fireEvent.click(within(placesGroup()).getByRole('button', { name: 'Places' }));
		expect(spy).not.toHaveBeenCalled();
	});
});

describe('the Places / Folders switch', () => {
	it('shows Places and Favourites first, and the Folders tree alone after the switch', async () => {
		await setup();
		const tabs = within(sidebar()).getAllByRole('tab');
		expect(tabs.map((tab) => tab.textContent)).toEqual(['Places', 'Folders']);
		expect(tabs[0]).toHaveAttribute('aria-selected', 'true');
		expect(within(sidebar()).queryByRole('tree')).toBeNull();
		showFolders();
		expect(tree()).toBeInTheDocument();
		expect(within(sidebar()).queryByRole('group', { name: 'Places' })).toBeNull();
		expect(within(sidebar()).getByRole('tab', { name: 'Folders' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
	});

	it('moves between the two with Left and Right and keeps one tab stop', async () => {
		await setup();
		const places = within(sidebar()).getByRole('tab', { name: 'Places' });
		expect(places).toHaveAttribute('tabindex', '0');
		expect(within(sidebar()).getByRole('tab', { name: 'Folders' })).toHaveAttribute(
			'tabindex',
			'-1',
		);
		places.focus();
		press(places, 'ArrowRight');
		expect(within(sidebar()).getByRole('tab', { name: 'Folders' })).toHaveFocus();
		expect(tree()).toBeInTheDocument();
		press(within(sidebar()).getByRole('tab', { name: 'Folders' }), 'ArrowLeft');
		expect(within(sidebar()).getByRole('tab', { name: 'Places' })).toHaveFocus();
	});
});

describe('navigating', () => {
	it('opens a place in the active tab on click', async () => {
		const h = await setup();
		fireEvent.click(within(placesGroup()).getByRole('button', { name: 'Downloads' }));
		await waitFor(async () => expect(await activeLocation(h)).toEqual(DOWNLOADS));
		expect(await screen.findByRole('option', { name: /^a\.zip/ })).toBeInTheDocument();
		expect(within(placesGroup()).getByRole('button', { name: 'Downloads' })).toHaveAttribute(
			'aria-current',
			'page',
		);
	});

	it('opens a middle-clicked item in a background tab', async () => {
		const h = await setup();
		const before = await h.tabs.getSnapshot();
		fireEvent.mouseDown(within(placesGroup()).getByRole('button', { name: 'Downloads' }), {
			button: 1,
		});
		fireEvent(
			within(placesGroup()).getByRole('button', { name: 'Downloads' }),
			new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true }),
		);
		await waitFor(async () => expect((await h.tabs.getSnapshot()).tabs).toHaveLength(2));
		const after = await h.tabs.getSnapshot();
		expect(after.active).toBe(before.active);
		expect(after.tabs[1]!.location).toEqual(DOWNLOADS);
	});

	it('offers Open and Open in New Tab on a place', async () => {
		const h = await setup();
		fireEvent.contextMenu(within(placesGroup()).getByRole('button', { name: 'Downloads' }), {
			clientX: 5,
			clientY: 5,
		});
		const menu = await screen.findByRole('menu', { name: 'Place actions' });
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((i) => i.textContent),
		).toEqual(['Open', 'Open in New Tab', 'Open in Split Pane', 'Open in New Window']);
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Open in New Tab' }));
		await waitFor(async () => expect((await h.tabs.getSnapshot()).tabs).toHaveLength(2));
		expect((await h.tabs.getSnapshot()).tabs[1]!.location).toEqual(DOWNLOADS);
	});

	it('opens the menu from the keyboard with the menu key and focuses into it', async () => {
		await setup();
		const button = within(placesGroup()).getByRole('button', { name: 'Music' });
		button.focus();
		press(button, 'ContextMenu');
		const menu = await screen.findByRole('menu', { name: 'Place actions' });
		await waitFor(() => expect(menu.contains(document.activeElement)).toBe(true));
	});

	it('moves focus between the items of a list with the arrow keys', async () => {
		await setup();
		const home = within(placesGroup()).getByRole('button', { name: 'Home' });
		home.focus();
		press(home, 'ArrowDown');
		expect(within(placesGroup()).getByRole('button', { name: 'Desktop' })).toHaveFocus();
		press(document.activeElement!, 'End');
		expect(within(placesGroup()).getByRole('button', { name: 'Trash' })).toHaveFocus();
		press(document.activeElement!, 'Home');
		// Overview is the first place, above Home.
		expect(within(placesGroup()).getByRole('button', { name: 'Overview' })).toHaveFocus();
		press(document.activeElement!, 'ArrowDown');
		expect(home).toHaveFocus();
	});
});

describe('favourites', () => {
	it('lists them, and a missing target shows the not-found state instead of crashing', async () => {
		const h = await setup(
			makePlaces([
				{ label: 'Projects', location: PROJECTS },
				{ label: 'Old stuff', location: GONE },
			]),
		);
		await within(favouritesGroup()).findByRole('button', { name: 'Old stuff' });
		fireEvent.click(within(favouritesGroup()).getByRole('button', { name: 'Old stuff' }));
		await waitFor(async () => expect(await activeLocation(h)).toEqual(GONE));
		expect(await screen.findByText('Folder not found')).toBeInTheDocument();
		expect(within(favouritesGroup()).getByRole('button', { name: 'Old stuff' })).toHaveAttribute(
			'aria-current',
			'page',
		);
		expect(sidebar()).toBeInTheDocument();
	});

	it('shows a hint while there are none', async () => {
		await setup(makePlaces([]));
		expect(within(favouritesGroup()).getByText(/No favourites yet/)).toBeInTheDocument();
	});

	it('adds a folder from the file list’s context menu, and only for folders', async () => {
		const h = await setup(makePlaces([]));
		fireEvent.contextMenu(await screen.findByRole('option', { name: /^notes\.txt/ }), {
			clientX: 10,
			clientY: 10,
		});
		expect(screen.queryByRole('menuitem', { name: 'Add to Favourites' })).toBeNull();
		fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());

		fireEvent.contextMenu(await screen.findByRole('option', { name: /^docs/ }), {
			clientX: 10,
			clientY: 10,
		});
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Add to Favourites' }));
		await waitFor(() => expect(h.places.calls).toContain(`add ${DOCS.uri}`));
		fireEvent.click(within(sidebar()).getByRole('tab', { name: 'Places' }));
		await within(favouritesGroup()).findByRole('button', { name: 'docs' });
		showFolders();
	});

	it('pins the current folder with Ctrl+D', async () => {
		await setup(makePlaces([]));
		press(window, 'd', { ctrlKey: true });
		await within(favouritesGroup()).findByRole('button', { name: 'test' });
	});

	it('removes a favourite from its menu', async () => {
		const h = await setup();
		fireEvent.contextMenu(
			await within(favouritesGroup()).findByRole('button', { name: 'Projects' }),
			{
				clientX: 5,
				clientY: 5,
			},
		);
		const menu = await screen.findByRole('menu', { name: 'Favourite actions' });
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((i) => i.textContent),
		).toEqual([
			'Open',
			'Open in New Tab',
			'Open in Split Pane',
			'Open in New Window',
			'RenameF2',
			'Move UpAlt+↑',
			'Move DownAlt+↓',
			'Remove from Favourites',
		]);
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Remove from Favourites' }));
		await waitFor(() =>
			expect(within(favouritesGroup()).queryByRole('button', { name: 'Projects' })).toBeNull(),
		);
		expect(h.places.calls).toContain(`remove ${PROJECTS.uri}`);
	});

	it('renames in place: Enter commits, Escape cancels, and clearing restores the folder’s name', async () => {
		const h = await setup();
		const projects = await within(favouritesGroup()).findByRole('button', { name: 'Projects' });
		projects.focus();
		press(projects, 'F2');
		let field = within(favouritesGroup()).getByRole('textbox', { name: 'Favourite name' });
		await waitFor(() => expect(field).toHaveFocus());
		fireEvent.change(field, { target: { value: 'Work' } });
		press(field, 'Enter');
		const renamed = await within(favouritesGroup()).findByRole('button', { name: 'Work' });
		await waitFor(() => expect(renamed).toHaveFocus());

		press(within(favouritesGroup()).getByRole('button', { name: 'Work' }), 'F2');
		field = within(favouritesGroup()).getByRole('textbox', { name: 'Favourite name' });
		fireEvent.change(field, { target: { value: 'Nope' } });
		press(field, 'Escape');
		expect(await within(favouritesGroup()).findByRole('button', { name: 'Work' })).toHaveFocus();
		expect(h.places.calls.filter((call) => call.startsWith('rename'))).toHaveLength(1);

		press(within(favouritesGroup()).getByRole('button', { name: 'Work' }), 'F2');
		field = within(favouritesGroup()).getByRole('textbox', { name: 'Favourite name' });
		fireEvent.change(field, { target: { value: '  ' } });
		press(field, 'Enter');
		await within(favouritesGroup()).findByRole('button', { name: 'projects' });
	});

	it('renames from the menu', async () => {
		await setup();
		fireEvent.contextMenu(
			await within(favouritesGroup()).findByRole('button', { name: 'Projects' }),
			{
				clientX: 5,
				clientY: 5,
			},
		);
		fireEvent.click(await screen.findByRole('menuitem', { name: /^Rename/ }));
		const field = await within(favouritesGroup()).findByRole('textbox', { name: 'Favourite name' });
		await waitFor(() => expect(field).toHaveFocus());
	});

	const order = () =>
		within(favouritesGroup())
			.getAllByRole('listitem')
			.map((row) => row.textContent);
	const three = () =>
		makePlaces([
			{ label: 'One', location: fileLocation('/srv/one') },
			{ label: 'Two', location: fileLocation('/srv/two') },
			{ label: 'Three', location: fileLocation('/srv/three') },
		]);

	it('reorders with Alt+Arrow keys and announces the new position', async () => {
		const places = three();
		await setup(places);
		const one = await within(favouritesGroup()).findByRole('button', { name: 'One' });
		one.focus();
		press(one, 'ArrowDown', { altKey: true });
		await waitFor(() => expect(order()).toEqual(['Two', 'One', 'Three']));
		await waitFor(() =>
			expect(within(sidebar()).getByText('Moved One to position 2 of 3')).toBeInTheDocument(),
		);
		press(within(favouritesGroup()).getByRole('button', { name: 'One' }), 'ArrowUp', {
			altKey: true,
		});
		await waitFor(() => expect(order()).toEqual(['One', 'Two', 'Three']));
		// Already first: there is nowhere to move it.
		const before = places.calls.length;
		press(within(favouritesGroup()).getByRole('button', { name: 'One' }), 'ArrowUp', {
			altKey: true,
		});
		expect(places.calls).toHaveLength(before);
		expect(order()).toEqual(['One', 'Two', 'Three']);
	});

	it('reorders by dragging one favourite onto another', async () => {
		await setup(three());
		const one = await within(favouritesGroup()).findByRole('button', { name: 'One' });
		const three_ = within(favouritesGroup()).getByRole('button', { name: 'Three' });
		const dataTransfer = { setData: vi.fn(), effectAllowed: '', dropEffect: '' };
		fireEvent.dragStart(one, { dataTransfer });
		fireEvent.dragOver(three_, { dataTransfer });
		expect(three_).toHaveAttribute('data-drop-target');
		fireEvent.drop(three_, { dataTransfer });
		await waitFor(() => expect(order()).toEqual(['Two', 'Three', 'One']));
		expect(three_).not.toHaveAttribute('data-drop-target');
	});

	it('reorders from the menu, with the ends disabled', async () => {
		await setup(three());
		fireEvent.contextMenu(await within(favouritesGroup()).findByRole('button', { name: 'One' }), {
			clientX: 5,
			clientY: 5,
		});
		const menu = await screen.findByRole('menu', { name: 'Favourite actions' });
		expect(within(menu).getByRole('menuitem', { name: /^Move Up/ })).toHaveAttribute(
			'aria-disabled',
			'true',
		);
		fireEvent.click(within(menu).getByRole('menuitem', { name: /^Move Down/ }));
		await waitFor(() => expect(order()).toEqual(['Two', 'One', 'Three']));
	});

	it('reports a failed change in the status bar and leaves the list as it was', async () => {
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		const places = three();
		await setup(places);
		await within(favouritesGroup()).findByRole('button', { name: 'One' });
		places.failNext({ kind: 'io', message: 'read-only', location: null });
		press(window, 'd', { ctrlKey: true });
		expect(await screen.findByText('Could not change the favourites.')).toBeInTheDocument();
		expect(order()).toEqual(['One', 'Two', 'Three']);
	});

	it('keeps a newer list when an older focus read resolves late', async () => {
		const places = makePlaces([]);
		await setup(places);
		const stale = await places.list();
		let release: (value: typeof stale) => void = () => {};
		vi.spyOn(places, 'list').mockImplementation(
			() => new Promise((resolve) => (release = resolve)),
		);
		act(() => {
			window.dispatchEvent(new Event('focus'));
		});
		await places.addFavourite(fileLocation('/srv/fresh'));
		await within(favouritesGroup()).findByRole('button', { name: 'fresh' });
		await act(async () => release(stale));
		expect(within(favouritesGroup()).getByRole('button', { name: 'fresh' })).toBeInTheDocument();
	});

	it('does not pin the folder on Ctrl+D typed in a text field', async () => {
		const h = await setup(makePlaces([]));
		const field = document.body.appendChild(document.createElement('input'));
		press(field, 'd', { ctrlKey: true });
		field.remove();
		expect(h.places.calls.filter((call) => call.startsWith('add'))).toEqual([]);
	});

	it('says the favourites failed, not that the folder could not open, from the file menu', async () => {
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		const places = makePlaces([]);
		await setup(places);
		fireEvent.contextMenu(await screen.findByRole('option', { name: /^docs/ }), {
			clientX: 10,
			clientY: 10,
		});
		places.failNext({ kind: 'io', message: 'read-only', location: null });
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Add to Favourites' }));
		expect(await screen.findByText('Could not change the favourites.')).toBeInTheDocument();
		expect(screen.queryByText(/Could not open/)).toBeNull();
	});

	it('picks up a change made elsewhere when the window regains focus', async () => {
		const places = makePlaces([]);
		await setup(places);
		await places.addFavourite(fileLocation('/srv/other'));
		await within(favouritesGroup()).findByRole('button', { name: 'other' });
		// Another program rewrites the bookmarks file: the window hears nothing until it looks again.
		const outside = new FakePlacesClient();
		await outside.addFavourite(fileLocation('/srv/elsewhere'));
		vi.spyOn(places, 'list').mockImplementation(() => outside.list());
		act(() => {
			window.dispatchEvent(new Event('focus'));
		});
		await within(favouritesGroup()).findByRole('button', { name: 'elsewhere' });
	});
});

describe('the Folders tree', () => {
	it('is a tree whose items carry level, size and position, open down to the current folder', async () => {
		await setup();
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		const root = item('/');
		expect(root).toHaveAttribute('aria-level', '1');
		expect(root).toHaveAttribute('aria-expanded', 'true');
		const home = item('home');
		expect(home).toHaveAttribute('aria-level', '2');
		expect(home).toHaveAttribute('aria-expanded', 'true');
		const test = item('test');
		expect(test).toHaveAttribute('aria-level', '3');
		expect(test).toHaveAttribute('aria-setsize', '1');
		expect(test).toHaveAttribute('aria-posinset', '1');
		expect(test).toHaveAttribute('aria-current', 'page');
		expect(test).toHaveAttribute('aria-expanded', 'false');
		expect(home).not.toHaveAttribute('aria-current');
	});

	it('expands with Left in a right-to-left layout', async () => {
		await setup();
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		document.documentElement.style.direction = 'rtl';
		try {
			const test = item('test');
			test.focus();
			press(test, 'ArrowLeft');
			expect(await within(tree()).findByRole('treeitem', { name: 'docs' })).toBeInTheDocument();
			expect(item('test')).toHaveAttribute('aria-expanded', 'true');
		} finally {
			document.documentElement.style.removeProperty('direction');
		}
	});

	it('loads a folder’s children only when it is expanded, and closes them when it collapses', async () => {
		const h = await setup();
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		// `/`, `/home` and the tab's own folder listing: the tree holds two, the tab one.
		await waitFor(() => expect(h.client.openCount).toBe(3));
		expect(within(tree()).queryByRole('treeitem', { name: 'docs' })).toBeNull();
		const test = item('test');
		test.focus();
		press(test, 'ArrowRight');
		expect(await within(tree()).findByRole('treeitem', { name: 'docs' })).toBeInTheDocument();
		expect(within(tree()).getByRole('treeitem', { name: 'music' })).toBeInTheDocument();
		// Files never appear in it.
		expect(within(tree()).queryByRole('treeitem', { name: 'notes.txt' })).toBeNull();
		expect(item('test')).toHaveAttribute('aria-expanded', 'true');
		await waitFor(() => expect(h.client.openCount).toBe(4));
		press(item('test'), 'ArrowLeft');
		await waitFor(() =>
			expect(within(tree()).queryByRole('treeitem', { name: 'docs' })).toBeNull(),
		);
		await waitFor(() => expect(h.client.openCount).toBe(3));
	});

	it('follows the active tab to another folder and marks it', async () => {
		const h = await setup();
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		await act(async () => {
			await h.tabs.navigate((await h.tabs.getSnapshot()).active!, DOCS);
		});
		const docs = await within(tree()).findByRole('treeitem', { name: 'docs' });
		expect(docs).toHaveAttribute('aria-current', 'page');
		expect(docs).toHaveAttribute('aria-level', '4');
		expect(item('test')).not.toHaveAttribute('aria-current');
		expect(item('test')).toHaveAttribute('aria-expanded', 'true');
	});

	it('navigates the tab on click and on Enter, and middle-click opens a background tab', async () => {
		const h = await setup();
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		press(item('test'), 'ArrowRight');
		const docs = await within(tree()).findByRole('treeitem', { name: 'docs' });
		fireEvent.click(docs);
		await waitFor(async () => expect(await activeLocation(h)).toEqual(DOCS));
		const music = await within(tree()).findByRole('treeitem', { name: 'music' });
		fireEvent(music, new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true }));
		await waitFor(async () => expect((await h.tabs.getSnapshot()).tabs).toHaveLength(2));
		expect((await h.tabs.getSnapshot()).tabs[1]!.location).toEqual(MUSIC);
		expect(await activeLocation(h)).toEqual(DOCS);
		music.focus();
		press(music, 'Enter');
		await waitFor(async () => expect(await activeLocation(h)).toEqual(MUSIC));
	});

	it('has one tab stop that roves with the arrow keys, Home, End and the tree’s levels', async () => {
		await setup();
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		const stops = () =>
			within(tree())
				.getAllByRole('treeitem')
				.filter((row) => row.tabIndex === 0)
				.map((row) => row.textContent);
		expect(stops()).toEqual(['test']);
		const test = item('test');
		test.focus();
		press(test, 'ArrowUp');
		expect(item('home')).toHaveFocus();
		expect(stops()).toEqual(['home']);
		press(item('home'), 'Home');
		expect(item('/')).toHaveFocus();
		press(item('/'), 'End');
		expect(item('test')).toHaveFocus();
		press(item('test'), 'ArrowLeft');
		expect(item('home')).toHaveFocus();
		press(item('home'), 'ArrowLeft');
		expect(item('home')).toHaveAttribute('aria-expanded', 'false');
		press(item('home'), 'ArrowRight');
		expect(item('home')).toHaveAttribute('aria-expanded', 'true');
		await within(tree()).findByRole('treeitem', { name: 'test' });
		press(item('home'), 'ArrowRight');
		expect(item('test')).toHaveFocus();
	});

	it('jumps to a row by its first letters', async () => {
		await setup();
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		press(item('test'), 'ArrowRight');
		await within(tree()).findByRole('treeitem', { name: 'music' });
		item('test').focus();
		press(item('test'), 'm');
		expect(item('music')).toHaveFocus();
		await new Promise((resolve) => setTimeout(resolve, 850));
		press(item('music'), 'd');
		expect(item('docs')).toHaveFocus();
	});

	it('offers Open, Open in New Tab and Add to Favourites on a folder', async () => {
		const h = await setup(makePlaces([]));
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		press(item('test'), 'ArrowRight');
		fireEvent.contextMenu(await within(tree()).findByRole('treeitem', { name: 'docs' }), {
			clientX: 5,
			clientY: 5,
		});
		const menu = await screen.findByRole('menu', { name: 'Folder actions' });
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((i) => i.textContent),
		).toEqual([
			'Open',
			'Open in New Tab',
			'Open in Split Pane',
			'Open in New Window',
			'Add to Favourites',
		]);
		fireEvent.click(within(menu).getByRole('menuitem', { name: 'Add to Favourites' }));
		await waitFor(() => expect(h.places.calls).toContain(`add ${DOCS.uri}`));
		fireEvent.click(within(sidebar()).getByRole('tab', { name: 'Places' }));
		await within(favouritesGroup()).findByRole('button', { name: 'docs' });
		showFolders();
		// Already pinned: nothing left to add.
		fireEvent.contextMenu(await within(tree()).findByRole('treeitem', { name: 'docs' }), {
			clientX: 5,
			clientY: 5,
		});
		expect(await screen.findByRole('menuitem', { name: 'Add to Favourites' })).toHaveAttribute(
			'aria-disabled',
			'true',
		);
	});

	it('lists hidden folders only while the tab shows hidden files', async () => {
		const h = await setup();
		h.client.setFolder(HOME, [
			...(await (async () => [makeEntry(1, 'docs', { kind: 'directory' })])()),
			makeEntry(2, '.config', { kind: 'directory' }),
		]);
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		press(item('test'), 'ArrowRight');
		await within(tree()).findByRole('treeitem', { name: 'docs' });
		expect(within(tree()).queryByRole('treeitem', { name: '.config' })).toBeNull();
		press(window, 'h', { ctrlKey: true });
		expect(await within(tree()).findByRole('treeitem', { name: '.config' })).toBeInTheDocument();
		press(window, 'h', { ctrlKey: true });
		await waitFor(() =>
			expect(within(tree()).queryByRole('treeitem', { name: '.config' })).toBeNull(),
		);
	});

	it('closes every listing it holds when the Places view is shown, and when the sidebar hides', async () => {
		const h = await setup();
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		await waitFor(() => expect(h.client.openCount).toBe(3));
		fireEvent.click(within(sidebar()).getByRole('tab', { name: 'Places' }));
		await waitFor(() => expect(h.client.openCount).toBe(1));
		showFolders();
		await waitFor(() => expect(h.client.openCount).toBe(3));
		press(window, 'F9');
		await waitFor(() => expect(h.client.openCount).toBe(1));
	});

	it('keeps the tree open at the ancestors when the section is reopened', async () => {
		await setup();
		showFolders();
		await screen.findByRole('treeitem', { name: 'test' });
		press(item('test'), 'ArrowRight');
		await within(tree()).findByRole('treeitem', { name: 'docs' });
		press(window, 'F9');
		press(window, 'F9');
		expect(await within(tree()).findByRole('treeitem', { name: 'docs' })).toBeInTheDocument();
		expect(item('test')).toHaveAttribute('aria-expanded', 'true');
	});
});
