// Verifies the clipboard and pane operations in the whole browsing area: the keys and menus, the dimmed rows, a pair's F5 and the destination dialog
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { Workspace } from '../app/Workspace';
import { dismissNotice } from '../app/notices';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakeOsClipboardClient } from '../services/fakeOsClipboardClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import type { FakeVfsClient } from '../services/fakeVfsClient';
import { PlacesClientProvider } from '../sidebar/PlacesClientContext';
import { TabsProvider } from '../tabs/TabsContext';
import { TrashClientProvider } from '../trash/TrashClientContext';
import { stubLayout } from '../test/browseHarness';
import { createTree, DOCS, HOME } from '../test/workspaceHarness';
import { recentDestinations } from './destinationModel';
import { MainOps } from './MainOps';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
	localStorage.clear();
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
});

interface Rendered {
	vfs: FakeVfsClient;
	ops: FakeOpsClient;
	os: FakeOsClipboardClient;
	tabs: FakeTabsApi;
}

async function mountWorkspace(options: { paired?: boolean } = {}): Promise<Rendered> {
	const vfs = createTree();
	const tabs = new FakeTabsApi();
	await tabs.openTab(HOME);
	if (options.paired) {
		const second = await tabs.openTab(DOCS);
		const first = (await tabs.getSnapshot()).tabs[0]!.id;
		await tabs.joinPair([first, second], 'sideBySide');
		await tabs.activateTab(first);
	}
	const ops = createFakeOpsClient({
		resolveSelection: async (handle, spec) =>
			Promise.all(spec.ids.map((id) => vfs.entryLocation(handle, id))),
	});
	const os = new FakeOsClipboardClient();
	const places = new FakePlacesClient({ places: fakePlaces('/home/test') });
	render(
		<VfsClientProvider client={vfs}>
			<PlacesClientProvider client={places}>
				<TrashClientProvider client={undefined}>
					<TabsProvider api={tabs} home={HOME}>
						<MainOps client={ops} osClipboard={os}>
							<Workspace />
						</MainOps>
					</TabsProvider>
				</TrashClientProvider>
			</PlacesClientProvider>
		</VfsClientProvider>,
	);
	// The sidebar's folder tree holds listings of its own; hide it so the panes are all there is.
	fireEvent.keyDown(window, { key: 'F9' });
	return { vfs, ops, os, tabs };
}

const options = (pane = 0) =>
	within(screen.getAllByRole('listbox', { name: 'Files' })[pane]!).getAllByRole('option');
const row = (name: string, pane = 0) =>
	options(pane).find((option) => option.textContent?.includes(name))!;
const submits = (ops: FakeOpsClient) => ops.calls.filter((call) => call[0] === 'submit');
const lastSubmit = (ops: FakeOpsClient) => submits(ops).at(-1)![1] as Record<string, unknown>;

async function choose(name: string, pane = 0) {
	await waitFor(() => expect(row(name, pane)).toBeDefined());
	fireEvent.click(row(name, pane));
	await waitFor(() => expect(row(name, pane)).toHaveAttribute('aria-selected', 'true'));
}

const keyOn = (_name: string, key: string, init: KeyboardEventInit = {}, pane = 0) =>
	fireEvent.keyDown(screen.getAllByRole('listbox', { name: 'Files' })[pane]!, { key, ...init });

describe('Ctrl+C, Ctrl+X and Ctrl+V', () => {
	it('copy a file and paste it into the other folder as a copy job', async () => {
		const { ops, os } = await mountWorkspace();
		await choose('notes.txt');
		await act(async () => keyOn('notes.txt', 'c', { ctrlKey: true }));
		await waitFor(() => expect(os.files?.uris).toEqual(['file:///home/test/notes.txt']));
		await waitFor(() => expect(screen.getByText('Copied 1 item')).toBeInTheDocument());
		// Paste into the folder that has the music: open it first.
		fireEvent.doubleClick(row('music'));
		await waitFor(() => expect(row('song.mp3')).toBeDefined());
		await act(async () => keyOn('song.mp3', 'v', { ctrlKey: true }));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'copy' },
			sources: { kind: 'locations', locations: [{ uri: 'file:///home/test/notes.txt' }] },
			destination: { uri: 'file:///home/test/music' },
		});
	});

	it('dim a cut row until the cut is pasted, and move on paste', async () => {
		const { ops } = await mountWorkspace();
		await choose('photo.jpg');
		await act(async () => keyOn('photo.jpg', 'x', { ctrlKey: true }));
		await waitFor(() => expect(row('photo.jpg')).toHaveAttribute('data-cut'));
		expect(row('notes.txt')).not.toHaveAttribute('data-cut');
		fireEvent.doubleClick(row('docs'));
		await waitFor(() => expect(row('report.pdf')).toBeDefined());
		// The cut was in the other folder: nothing is dimmed here.
		expect(row('report.pdf')).not.toHaveAttribute('data-cut');
		await act(async () => keyOn('report.pdf', 'v', { ctrlKey: true }));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'move' },
			destination: { uri: 'file:///home/test/docs' },
		});
		// Spent: back in the first folder the row is whole again.
		await waitFor(async () => expect((await ops.getClipboard()).items).toEqual([]));
	});

	it('say what a copy or cut took, and that there is nothing to paste', async () => {
		await mountWorkspace();
		await waitFor(() => expect(options()).not.toHaveLength(0));
		await act(async () => keyOn('x', 'v', { ctrlKey: true }));
		await waitFor(() => expect(screen.getByText('There is nothing to paste.')).toBeInTheDocument());
		await choose('notes.txt');
		await act(async () => keyOn('notes.txt', 'x', { ctrlKey: true }));
		await waitFor(() => expect(screen.getByText('Cut 1 item')).toBeInTheDocument());
	});

	it('paste files another application copied', async () => {
		const { ops, os } = await mountWorkspace();
		await waitFor(() => expect(options()).not.toHaveLength(0));
		os.external({ uris: ['file:///mnt/other/z.txt'], cut: false });
		await act(async () => keyOn('x', 'v', { ctrlKey: true }));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'copy' },
			sources: { locations: [{ uri: 'file:///mnt/other/z.txt' }] },
			destination: { uri: 'file:///home/test' },
		});
	});
});

describe('the menus', () => {
	it('offer Cut, Copy, Paste and the destination dialog on a file, and Paste Into Folder on a folder', async () => {
		const user = userEvent.setup();
		await mountWorkspace();
		await choose('notes.txt');
		fireEvent.contextMenu(row('notes.txt'));
		let menu = await screen.findByRole('menu', { name: 'Item actions' });
		for (const name of [/^Cut/, /^Copy Ctrl/, /^Paste/, /^Copy To…/, /^Move To…/]) {
			expect(within(menu).getByRole('menuitem', { name })).toBeInTheDocument();
		}
		expect(within(menu).queryByRole('menuitem', { name: /Other Pane/ })).toBeNull();
		await user.keyboard('{Escape}');
		fireEvent.contextMenu(row('docs'));
		menu = await screen.findByRole('menu', { name: 'Item actions' });
		expect(within(menu).getByRole('menuitem', { name: 'Paste Into Folder' })).toBeInTheDocument();
		expect(within(menu).queryByRole('menuitem', { name: /^Paste(?! Into)/ })).toBeNull();
	});

	it('offer Paste after New in the empty-space menu', async () => {
		await mountWorkspace();
		await waitFor(() => expect(options()).not.toHaveLength(0));
		fireEvent.contextMenu(screen.getAllByRole('listbox', { name: 'Files' })[0]!.parentElement!);
		const menu = await screen.findByRole('menu', { name: 'Folder actions' });
		const items = within(menu).getAllByRole('menuitem');
		const at = (name: RegExp) => items.findIndex((item) => name.test(item.textContent ?? ''));
		expect(at(/^New/)).toBeGreaterThanOrEqual(0);
		expect(at(/^Paste/)).toBe(at(/^New/) + 1);
	});
});

describe('Copy To… and Move To…', () => {
	it('open the dialog from the item menu and copy the selection to the folder chosen, remembering it', async () => {
		const user = userEvent.setup();
		const { ops } = await mountWorkspace();
		await choose('notes.txt');
		fireEvent.contextMenu(row('notes.txt'));
		await user.click(await screen.findByRole('menuitem', { name: /Copy To…/ }));
		const dialog = await screen.findByRole('dialog', { name: 'Copy 1 item to…' });
		const field = within(dialog).getByRole('textbox', { name: 'Folder' });
		expect(field).toHaveFocus();
		await user.clear(field);
		await user.type(field, '/home/test/music');
		const copy = within(dialog).getByRole('button', { name: 'Copy' });
		await waitFor(() => expect(copy).toBeEnabled());
		await user.click(copy);
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'copy' },
			destination: { uri: 'file:///home/test/music' },
			sources: { kind: 'selection' },
		});
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(recentDestinations.list()[0]?.uri).toBe('file:///home/test/music');
	});

	it('submit nothing when the dialog is cancelled with Esc', async () => {
		const user = userEvent.setup();
		const { ops } = await mountWorkspace();
		await choose('notes.txt');
		fireEvent.contextMenu(row('notes.txt'));
		await user.click(await screen.findByRole('menuitem', { name: /Move To…/ }));
		await screen.findByRole('dialog', { name: 'Move 1 item to…' });
		await user.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(submits(ops)).toHaveLength(0);
	});

	it('offer places, favourites and open tabs to choose from', async () => {
		const user = userEvent.setup();
		await mountWorkspace();
		await choose('notes.txt');
		fireEvent.contextMenu(row('notes.txt'));
		await user.click(await screen.findByRole('menuitem', { name: /Copy To…/ }));
		const dialog = await screen.findByRole('dialog');
		expect(within(dialog).getByRole('region', { name: 'Places' })).toBeInTheDocument();
		expect(within(dialog).getByRole('region', { name: 'Open tabs' })).toBeInTheDocument();
	});
});

describe('F5 and Shift+F5', () => {
	it('copy and move to the other pane of a pair without asking', async () => {
		const { ops } = await mountWorkspace({ paired: true });
		await waitFor(() => expect(row('notes.txt', 0)).toBeDefined());
		await choose('notes.txt', 0);
		await act(async () => keyOn('notes.txt', 'F5', {}, 0));
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'copy' },
			destination: { uri: 'file:///home/test/docs' },
		});
		await act(async () => keyOn('notes.txt', 'F5', { shiftKey: true }, 0));
		await waitFor(() => expect(submits(ops)).toHaveLength(2));
		expect(lastSubmit(ops)).toMatchObject({
			kind: { kind: 'move' },
			destination: { uri: 'file:///home/test/docs' },
		});
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('are in the menu as Copy to Other Pane and Move to Other Pane, with their keys, in a pair only', async () => {
		const user = userEvent.setup();
		const { ops } = await mountWorkspace({ paired: true });
		await waitFor(() => expect(row('notes.txt', 0)).toBeDefined());
		await choose('notes.txt', 0);
		fireEvent.contextMenu(row('notes.txt', 0));
		const copy = await screen.findByRole('menuitem', { name: /Copy to Other Pane/ });
		expect(copy).toHaveTextContent('F5');
		expect(screen.getByRole('menuitem', { name: /Move to Other Pane/ })).toHaveTextContent(
			'Shift+F5',
		);
		await user.click(copy);
		await waitFor(() => expect(submits(ops)).toHaveLength(1));
	});

	it('open Copy To… without a pair, and do nothing but say so with nothing selected', async () => {
		const { ops } = await mountWorkspace();
		await waitFor(() => expect(options()).not.toHaveLength(0));
		await act(async () => keyOn('x', 'F5'));
		await waitFor(() => expect(screen.getByText('Nothing is selected.')).toBeInTheDocument());
		expect(screen.queryByRole('dialog')).toBeNull();
		await choose('notes.txt');
		await act(async () => keyOn('notes.txt', 'F5'));
		await screen.findByRole('dialog', { name: 'Copy 1 item to…' });
		expect(submits(ops)).toHaveLength(0);
	});
});
