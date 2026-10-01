// Verifies workspaces in the browsing window: saving a group, the sidebar's Workspaces section, and the Favourites switch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { fileLocation } from '../services/fakeVfsClient';
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

const PROJECTS = fileLocation('/srv/projects');

async function setup() {
	const places = new FakePlacesClient({
		places: fakePlaces('/home/test'),
		favourites: [{ label: 'Projects', location: PROJECTS }],
	});
	const h = await renderWorkspace(createTree(), undefined, places, { sidebar: true });
	await screen.findByRole('button', { name: 'Home' });
	// Tabs test (1), docs (2), music (3), with 1 and 2 grouped as "Group 1".
	await h.tabs.openTab(DOCS);
	await h.tabs.openTab(MUSIC);
	await h.tabs.createGroup([1, 2]);
	await h.tabs.activateTab(1);
	await screen.findByRole('button', { name: /^Group 1,/ });
	return h;
}
type Harness = Awaited<ReturnType<typeof setup>>;

const sidebar = () => screen.getByRole('navigation', { name: 'Sidebar' });
const group = (name: string | RegExp) => within(sidebar()).getByRole('group', { name });
const workspacesGroup = () => group('Workspaces');
const favourites = () => group(/^Favourites/);
const workspaceItem = (name: string) =>
	within(workspacesGroup()).getByRole('button', { name }) as HTMLElement;
const names = (root: HTMLElement) =>
	within(root)
		.getAllByRole('button')
		.slice(1)
		.map((b) => b.textContent);
const chip = () => screen.getByRole('button', { name: /^Group 1,/ });

async function saveFromMenu() {
	fireEvent.contextMenu(chip());
	fireEvent.click(await screen.findByRole('menuitem', { name: 'Save Group as Workspace' }));
}

describe('Save Group as Workspace', () => {
	it('saves the group’s folders under the group’s name and says so, without switching', async () => {
		const h = await setup();
		await saveFromMenu();
		await waitFor(async () => expect((await h.tabs.getSnapshot()).workspaces).toHaveLength(1));
		const saved = (await h.tabs.getSnapshot()).workspaces[0]!;
		expect(saved.name).toBe('Group 1');
		expect(saved.locations).toEqual([HOME, DOCS]);
		expect(await screen.findByText(/^Saved workspace Group 1/)).toBeInTheDocument();
		expect(workspaceItem('Group 1')).not.toHaveAttribute('aria-current');
		expect(favourites()).toHaveAccessibleName('Favourites');
	});

	it('asks for another name, on the chip, when the name is taken', async () => {
		const h = await setup();
		await h.tabs.saveGroupAsWorkspace(1, 'Group 1');
		await saveFromMenu();
		const field = await screen.findByRole('textbox', { name: 'Workspace name' });
		expect(await screen.findByText(/already exists/)).toBeInTheDocument();
		fireEvent.change(field, { target: { value: 'Site' } });
		fireEvent.keyDown(field, { key: 'Enter' });
		await waitFor(async () =>
			expect((await h.tabs.getSnapshot()).workspaces.map((w) => w.name)).toEqual([
				'Group 1',
				'Site',
			]),
		);
		expect(screen.queryByRole('textbox', { name: 'Workspace name' })).toBeNull();
	});

	it('cancels with Escape and leaves nothing saved', async () => {
		const h = await setup();
		await h.tabs.saveGroupAsWorkspace(1, 'Group 1');
		await saveFromMenu();
		const field = await screen.findByRole('textbox', { name: 'Workspace name' });
		fireEvent.keyDown(field, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
		expect((await h.tabs.getSnapshot()).workspaces).toHaveLength(1);
	});
});

describe('the Workspaces section', () => {
	it('is a labelled group after Favourites, with a hint while there are none', async () => {
		await setup();
		const headings = within(sidebar())
			.getAllByRole('heading', { level: 2 })
			.map((h) => h.textContent);
		expect(headings).toEqual(['Places', 'Favourites', 'Workspaces']);
		expect(within(workspacesGroup()).getByText(/No workspaces yet/)).toBeInTheDocument();
	});

	it('lists the workspaces after None, and marks the one in use', async () => {
		const h = await setup();
		await h.tabs.saveGroupAsWorkspace(1, 'Site');
		await within(workspacesGroup()).findByRole('button', { name: 'Site' });
		expect(names(workspacesGroup())).toEqual(['None', 'Site']);
		expect(workspaceItem('None')).toHaveAttribute('aria-current', 'true');
	});

	it('switches the Favourites to the workspace and back to the bookmarks', async () => {
		const h = await setup();
		await h.tabs.saveGroupAsWorkspace(1, 'Site');
		fireEvent.click(await within(workspacesGroup()).findByRole('button', { name: 'Site' }));
		await within(sidebar()).findByRole('group', { name: 'Favourites · Site' });
		expect(names(favourites())).toEqual(['test', 'docs']);
		expect(workspaceItem('Site')).toHaveAttribute('aria-current', 'true');
		expect(workspaceItem('None')).not.toHaveAttribute('aria-current');
		expect(await screen.findByText('Favourites now show workspace Site')).toBeInTheDocument();
		expect((await h.tabs.getSnapshot()).workspace).toBe(1);

		fireEvent.click(workspaceItem('None'));
		await within(sidebar()).findByRole('group', { name: 'Favourites' });
		expect(names(favourites())).toEqual(['Projects']);
		expect(await screen.findByText('Favourites show your bookmarks')).toBeInTheDocument();
	});

	it('edits the workspace, never the bookmarks, while one is showing', async () => {
		const h = await setup();
		await h.tabs.saveGroupAsWorkspace(1, 'Site');
		await h.tabs.setActiveWorkspace(1);
		await within(sidebar()).findByRole('group', { name: 'Favourites · Site' });

		// Add: a folder in the list's context menu.
		fireEvent.contextMenu(await screen.findByRole('option', { name: /^music/ }), {
			clientX: 10,
			clientY: 10,
		});
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Add to Favourites' }));
		await waitFor(() => expect(names(favourites())).toEqual(['test', 'docs', 'music']));

		// Reorder with Alt+Down, then remove from the row's menu.
		const row = within(favourites()).getByRole('button', { name: 'test' });
		fireEvent.keyDown(row, { key: 'ArrowDown', altKey: true });
		await waitFor(() => expect(names(favourites())).toEqual(['docs', 'test', 'music']));
		fireEvent.contextMenu(within(favourites()).getByRole('button', { name: 'docs' }));
		expect(screen.queryByRole('menuitem', { name: 'Rename' })).toBeNull();
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Remove from Favourites' }));
		await waitFor(() => expect(names(favourites())).toEqual(['test', 'music']));
		expect((await h.tabs.getSnapshot()).workspaces[0]!.locations).toEqual([HOME, MUSIC]);

		// F2 does nothing: workspace folders have no labels of their own.
		fireEvent.keyDown(within(favourites()).getByRole('button', { name: 'test' }), { key: 'F2' });
		expect(screen.queryByRole('textbox')).toBeNull();
		expect(h.places.calls.filter((call) => /^(add|remove|move|rename)/.test(call))).toEqual([]);

		fireEvent.click(workspaceItem('None'));
		await within(sidebar()).findByRole('group', { name: 'Favourites' });
		expect(names(favourites())).toEqual(['Projects']);
	});

	it('opens a missing workspace folder as the usual Folder not found', async () => {
		const h = await setup();
		await h.tabs.saveGroupAsWorkspace(1, 'Site');
		await h.tabs.setWorkspaceLocations(1, [fileLocation('/srv/gone')]);
		await h.tabs.setActiveWorkspace(1);
		fireEvent.click(await within(favourites()).findByRole('button', { name: 'gone' }));
		expect(await screen.findByText('Folder not found')).toBeInTheDocument();
	});

	it('follows the session: deleting the workspace in use shows the bookmarks again', async () => {
		const h = await setup();
		await h.tabs.saveGroupAsWorkspace(1, 'Site');
		await h.tabs.setActiveWorkspace(1);
		await within(sidebar()).findByRole('group', { name: 'Favourites · Site' });
		await h.tabs.deleteWorkspace(1);
		await within(sidebar()).findByRole('group', { name: 'Favourites' });
		expect(names(favourites())).toEqual(['Projects']);
	});
});

describe('a workspace item', () => {
	async function withWorkspace(): Promise<Harness> {
		const h = await setup();
		await h.tabs.saveGroupAsWorkspace(1, 'Site');
		await within(workspacesGroup()).findByRole('button', { name: 'Site' });
		return h;
	}

	it('opens all its folders as tabs from its menu, the first one current', async () => {
		const h = await withWorkspace();
		fireEvent.contextMenu(workspaceItem('Site'), { clientX: 5, clientY: 5 });
		expect(await screen.findByRole('menu', { name: 'Workspace actions' })).toBeInTheDocument();
		fireEvent.click(screen.getByRole('menuitem', { name: 'Open All in Tabs' }));
		await waitFor(async () => expect((await h.tabs.getSnapshot()).tabs).toHaveLength(5));
		const snapshot = await h.tabs.getSnapshot();
		// The two folders land right after the tab that was active, in order.
		expect(snapshot.tabs.map((tab) => tab.location.display)).toEqual([
			HOME.display,
			HOME.display,
			DOCS.display,
			DOCS.display,
			MUSIC.display,
		]);
		expect(snapshot.active).toBe(snapshot.tabs[1]!.id);
		expect(await screen.findByText('Opened 2 folders from Site')).toBeInTheDocument();
	});

	it('says exactly how far it got when a folder fails to open, keeping the tabs already open', async () => {
		const h = await withWorkspace();
		const real = h.tabs.openTab.bind(h.tabs);
		let calls = 0;
		vi.spyOn(h.tabs, 'openTab').mockImplementation(async (location, options) => {
			if (++calls === 2) throw new Error('boom');
			return real(location, options);
		});
		fireEvent.contextMenu(workspaceItem('Site'), { clientX: 5, clientY: 5 });
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Open All in Tabs' }));
		expect((await screen.findAllByText('Opened 1 of 2 folders from Site')).length).toBeGreaterThan(
			0,
		);
		expect((await h.tabs.getSnapshot()).tabs).toHaveLength(4);
	});

	it('renames with F2 and asks again for a name that is taken', async () => {
		const h = await withWorkspace();
		await h.tabs.saveGroupAsWorkspace(1, 'Other');
		await within(workspacesGroup()).findByRole('button', { name: 'Other' });
		act(() => workspaceItem('Site').focus());
		fireEvent.keyDown(workspaceItem('Site'), { key: 'F2' });
		let field = await screen.findByRole('textbox', { name: 'Workspace name' });
		fireEvent.change(field, { target: { value: ' other ' } });
		fireEvent.keyDown(field, { key: 'Enter' });
		field = await screen.findByRole('textbox', { name: 'Workspace name' });
		expect(await screen.findByText(/already exists/)).toBeInTheDocument();
		fireEvent.change(field, { target: { value: 'Renamed' } });
		fireEvent.keyDown(field, { key: 'Enter' });
		await waitFor(async () =>
			expect((await h.tabs.getSnapshot()).workspaces.map((w) => w.name)).toEqual([
				'Renamed',
				'Other',
			]),
		);
		expect(await screen.findByText('Renamed workspace to Renamed')).toBeInTheDocument();
		await waitFor(() => expect(document.activeElement).toBe(workspaceItem('Renamed')));
	});

	it('deletes with the Delete key and moves focus to the item above', async () => {
		const h = await withWorkspace();
		await h.tabs.saveGroupAsWorkspace(1, 'Other');
		await within(workspacesGroup()).findByRole('button', { name: 'Other' });
		act(() => workspaceItem('Other').focus());
		fireEvent.keyDown(workspaceItem('Other'), { key: 'Delete' });
		await waitFor(async () => expect((await h.tabs.getSnapshot()).workspaces).toHaveLength(1));
		expect(await screen.findByText('Deleted workspace Other')).toBeInTheDocument();
		await waitFor(() => expect(document.activeElement).toBe(workspaceItem('Site')));
	});

	it('moves focus with the arrow keys and opens its menu from the keyboard', async () => {
		await withWorkspace();
		act(() => workspaceItem('None').focus());
		fireEvent.keyDown(workspaceItem('None'), { key: 'ArrowDown' });
		expect(document.activeElement).toBe(workspaceItem('Site'));
		fireEvent.keyDown(workspaceItem('Site'), { key: 'F10', shiftKey: true });
		expect(await screen.findByRole('menu', { name: 'Workspace actions' })).toBeInTheDocument();
		fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
		await waitFor(() => expect(document.activeElement).toBe(workspaceItem('Site')));
	});
});
