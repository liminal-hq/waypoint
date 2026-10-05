// Verifies the destination dialog: its sections, the typed path and what it is checked as, New Folder…, the keyboard and focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import { FOLDER } from '../test/browseHarness';
import {
	DestinationDialog,
	type DestinationChoices,
	type DestinationDialogProps,
} from './DestinationDialog';

afterEach(cleanup);

const DOCS = fileLocation('/home/test/docs');
const ARCHIVE = fileLocation('/home/test/archive');
const REMOTE = fileLocation('/mnt/other');

function vfsWith() {
	const vfs = new FakeVfsClient({ home: '/home/test' });
	vfs.setFolder(FOLDER, [
		makeEntry(1, 'notes.txt'),
		makeEntry(2, 'docs', { kind: 'directory' }),
		makeEntry(3, 'archive', { kind: 'directory' }),
	]);
	vfs.setFolder(DOCS, []);
	vfs.setFolder(ARCHIVE, []);
	vfs.setFolder(REMOTE, []);
	vfs.setReadOnly(ARCHIVE);
	return vfs;
}

const choices: DestinationChoices = {
	places: [
		{ label: 'Home', location: FOLDER },
		{ label: 'Documents', location: DOCS },
	],
	favourites: [{ label: 'Archive', location: ARCHIVE }],
	tabs: [{ label: 'other', location: REMOTE }],
	recent: [{ label: '/home/test/docs', location: DOCS }],
};

function setup(overrides: Partial<DestinationDialogProps> = {}) {
	const onConfirm = vi.fn();
	const onCancel = vi.fn();
	const user = userEvent.setup();
	const vfs = vfsWith();
	render(
		<DestinationDialog
			options={{
				title: 'Copy 2 items to…',
				confirmLabel: 'Copy',
				base: FOLDER,
				origin: FOLDER,
			}}
			vfs={overrides.vfs ?? vfs}
			choices={choices}
			debounceMs={0}
			onConfirm={onConfirm}
			onCancel={onCancel}
			{...overrides}
		/>,
	);
	return { user, onConfirm, onCancel, vfs };
}

const field = () => screen.getByRole('textbox', { name: 'Folder' });
const confirm = () => screen.getByRole('button', { name: 'Copy' });
const status = () => document.querySelector('[data-state]') as HTMLElement;

describe('opening', () => {
	it('is a dialog named for the job, with focus in the path field holding the starting folder', async () => {
		setup();
		const dialog = await screen.findByRole('dialog', { name: 'Copy 2 items to…' });
		expect(dialog).toHaveAccessibleDescription('Type a path, or choose one below.');
		expect(field()).toHaveFocus();
		expect(field()).toHaveValue('/home/test');
	});

	it('starts from the folder it is told to, rather than the base', async () => {
		setup({
			options: { title: 'T', confirmLabel: 'Copy', base: FOLDER, initial: DOCS },
		});
		expect(field()).toHaveValue('/home/test/docs');
		await waitFor(() => expect(confirm()).toBeEnabled());
	});

	it('can start with an empty field, which waits for a folder and says so without calling it an error', async () => {
		const { user, onConfirm } = setup({
			options: { title: 'T', confirmLabel: 'Copy', base: FOLDER, initial: DOCS, startEmpty: true },
		});
		expect(field()).toHaveValue('');
		await waitFor(() =>
			expect(status()).toHaveTextContent('Type a folder, or pick one from the lists.'),
		);
		expect(field()).not.toHaveAttribute('aria-invalid');
		expect(confirm()).toBeDisabled();
		// Enter in the empty field chooses nothing.
		await user.type(field(), '{Enter}');
		expect(onConfirm).not.toHaveBeenCalled();
	});

	it('lists places, favourites, open tabs and recent folders, each under its own heading', async () => {
		setup();
		const names = (heading: string) =>
			within(screen.getByRole('region', { name: heading }))
				.getAllByRole('button')
				.map((button) => button.textContent);
		expect(names('Places')).toEqual(['Home/home/test', 'Documents/home/test/docs']);
		expect(names('Favourites')).toEqual(['Archive/home/test/archive']);
		expect(names('Open tabs')).toEqual(['other/mnt/other']);
		expect(names('Recent')).toEqual(['/home/test/docs/home/test/docs']);
	});

	it('leaves out a section with nothing in it', () => {
		setup({ choices: { ...choices, recent: [], tabs: [] } });
		expect(screen.queryByRole('region', { name: 'Recent' })).toBeNull();
		expect(screen.queryByRole('region', { name: 'Open tabs' })).toBeNull();
		expect(screen.getByRole('region', { name: 'Places' })).toBeInTheDocument();
		expect(screen.queryByRole('region', { name: 'Servers' })).toBeNull();
	});

	it('lists the servers after the favourites, at the folder each opens at', () => {
		const nas = { display: 'sftp://me@nas.lan/srv', uri: 'sftp://me@nas.lan/srv' };
		setup({ choices: { ...choices, servers: [{ label: 'NAS', location: nas }] } });
		const servers = screen.getByRole('region', { name: 'Servers' });
		expect(within(servers).getByRole('button').textContent).toBe('NASsftp://me@nas.lan/srv');
		const headings = screen
			.getAllByRole('region')
			.map((region) => region.getAttribute('aria-label') ?? region.textContent);
		expect(headings.findIndex((h) => h?.startsWith('Servers'))).toBe(
			headings.findIndex((h) => h?.startsWith('Favourites')) + 1,
		);
	});
});

describe('the typed path', () => {
	it('is checked, and the primary button waits for a folder that can be written to', async () => {
		const { user } = setup();
		await user.clear(field());
		await user.type(field(), '/home/test/docs');
		await waitFor(() => expect(status()).toHaveAttribute('data-state', 'ok'));
		expect(status()).toHaveTextContent('Ready: docs can be written to.');
		expect(confirm()).toBeEnabled();
	});

	it('says why a folder is not usable, in words, and keeps the button off', async () => {
		const { user } = setup();
		const check = async (text: string, message: string) => {
			await user.clear(field());
			await user.type(field(), text);
			await waitFor(() => expect(status()).toHaveTextContent(message));
			expect(status()).toHaveAttribute('data-state', 'problem');
			expect(status()).toHaveAttribute('role', 'alert');
			expect(field()).toHaveAttribute('aria-invalid', 'true');
			expect(confirm()).toBeDisabled();
		};
		await check('/home/test/missing', '“missing” does not exist.');
		await check('/home/test/notes.txt', '“notes.txt” is not a folder.');
		await check('/home/test/archive', '“archive” cannot be written to.');
		await check('sftp://host/x', 'Locations of the kind sftp cannot be used yet.');
	});

	it('reads relative text against the pane’s folder and ~ against home', async () => {
		const { user, onConfirm } = setup();
		await user.clear(field());
		await user.type(field(), 'docs{Enter}');
		await waitFor(() => expect(onConfirm).toHaveBeenCalledWith(DOCS));
		onConfirm.mockClear();
		await user.clear(field());
		await user.type(field(), '~/docs{Enter}');
		await waitFor(() => expect(onConfirm).toHaveBeenCalledWith(DOCS));
	});

	it('refuses the folder the items are in for a move, but not for a copy', async () => {
		const { user } = setup({
			options: {
				title: 'Move',
				confirmLabel: 'Copy',
				base: FOLDER,
				origin: FOLDER,
				forbidOrigin: true,
			},
		});
		await waitFor(() =>
			expect(status()).toHaveTextContent('The items are already in that folder.'),
		);
		expect(confirm()).toBeDisabled();
		await user.clear(field());
		await user.type(field(), 'docs');
		await waitFor(() => expect(confirm()).toBeEnabled());
	});
});

describe('choosing', () => {
	it('puts a chosen folder in the field, checks it and enables the button', async () => {
		const { user, onConfirm } = setup();
		await user.click(
			within(screen.getByRole('region', { name: 'Places' })).getByRole('button', {
				name: /Documents/,
			}),
		);
		expect(field()).toHaveValue('/home/test/docs');
		expect(field()).toHaveFocus();
		await waitFor(() => expect(confirm()).toBeEnabled());
		await user.click(confirm());
		expect(onConfirm).toHaveBeenCalledWith(DOCS);
	});

	it('shows a chosen folder that cannot be used as a problem rather than accepting it', async () => {
		const { user, onConfirm } = setup();
		await user.click(
			within(screen.getByRole('region', { name: 'Favourites' })).getByRole('button', {
				name: /Archive/,
			}),
		);
		await waitFor(() => expect(status()).toHaveTextContent('“archive” cannot be written to.'));
		await user.click(confirm());
		expect(onConfirm).not.toHaveBeenCalled();
	});

	it('chooses from open tabs and from recent folders as from places', async () => {
		const { user } = setup();
		await user.click(within(screen.getByRole('region', { name: 'Open tabs' })).getByRole('button'));
		expect(field()).toHaveValue('/mnt/other');
		await user.click(within(screen.getByRole('region', { name: 'Recent' })).getByRole('button'));
		expect(field()).toHaveValue('/home/test/docs');
	});
});

describe('the keyboard', () => {
	it('Enter in the field confirms once the folder checks out, without waiting for the pause', async () => {
		const { user, onConfirm } = setup({ debounceMs: 5_000 });
		await user.clear(field());
		await user.type(field(), '/home/test/docs{Enter}');
		await waitFor(() => expect(onConfirm).toHaveBeenCalledWith(DOCS));
		expect(onConfirm).toHaveBeenCalledTimes(1);
	});

	it('Enter on a folder that is not usable does not confirm, and says why', async () => {
		const { user, onConfirm } = setup();
		await user.clear(field());
		await user.type(field(), '/home/test/nope{Enter}');
		await waitFor(() => expect(status()).toHaveTextContent('does not exist'));
		expect(onConfirm).not.toHaveBeenCalled();
	});

	it('Esc and Cancel cancel', async () => {
		const { user, onCancel } = setup();
		await user.keyboard('{Escape}');
		expect(onCancel).toHaveBeenCalledTimes(1);
		await user.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(onCancel).toHaveBeenCalledTimes(2);
	});

	it('reaches every control with Tab, in the order of the page, and keeps focus in the dialog', async () => {
		const { user } = setup();
		expect(field()).toHaveFocus();
		await user.tab();
		expect(document.activeElement?.textContent).toMatch(/Home/);
		for (let i = 0; i < 12; i++) await user.tab();
		expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);
	});
});

describe('New Folder…', () => {
	it('is not offered without a way to make one', () => {
		setup();
		expect(screen.queryByRole('button', { name: 'New Folder…' })).toBeNull();
	});

	it('waits for a folder that can be written to, makes one in it through the queue and chooses it', async () => {
		const made = fileLocation('/home/test/docs/untitled folder');
		const createFolder = vi.fn(async () => made);
		const { user, vfs } = setup({ createFolder });
		vfs.setFolder(made, []);
		const button = screen.getByRole('button', { name: 'New Folder…' });
		expect(button).toBeDisabled();
		await user.clear(field());
		await user.type(field(), '/home/test/docs');
		await waitFor(() => expect(button).toBeEnabled());
		await user.click(button);
		expect(createFolder).toHaveBeenCalledWith(DOCS);
		await waitFor(() => expect(field()).toHaveValue('/home/test/docs/untitled folder'));
		await waitFor(() => expect(status()).toHaveAttribute('data-state', 'ok'));
		expect(confirm()).toBeEnabled();
	});

	it('says why when the folder could not be made, and keeps the dialog open', async () => {
		const createFolder = vi.fn(async () => {
			throw new Error('Permission denied for docs');
		});
		const { user, onConfirm } = setup({ createFolder });
		await user.clear(field());
		await user.type(field(), '/home/test/docs');
		const button = screen.getByRole('button', { name: 'New Folder…' });
		await waitFor(() => expect(button).toBeEnabled());
		await user.click(button);
		await waitFor(() =>
			expect(status()).toHaveTextContent('Could not make the folder: Permission denied for docs'),
		);
		expect(onConfirm).not.toHaveBeenCalled();
		expect(screen.getByRole('dialog')).toBeInTheDocument();
	});
});
