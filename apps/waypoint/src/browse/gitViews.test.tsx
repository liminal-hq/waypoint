// Verifies what Git adds to the views: the column with its letters and words, the header menu, sorting, the grid's chip and the folder badge
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { GitProvider } from '../git/GitContext';
import { createFakeGitClient, cleanSummary, type FakeGitClient } from '../services/fakeGitClient';
import { createFakeSettingsClient, type FakeSettings } from '../services/fakeSettingsClient';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { DEFAULT_SETTINGS, type Settings } from '../services/settingsClient';
import { SettingsProvider } from '../settings/SettingsContext';
import { FOLDER, stubLayout } from '../test/browseHarness';
import { FileIcon } from './FileIcon';
import { GridView } from './GridView';
import { ListView } from './ListView';
import { useListingSession } from './useListingSession';
import { useVfsClient, VfsClientProvider } from './VfsClientContext';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(400, 700);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

const entries = (): Entry[] => [
	makeEntry(1, 'clean.txt'),
	makeEntry(2, 'edited.txt', { git: { unstaged: 'modified' } }),
	makeEntry(3, 'added.txt', { git: { staged: 'added' } }),
	makeEntry(4, 'both.txt', { git: { staged: 'modified', unstaged: 'modified' } }),
	makeEntry(5, 'new.txt', { git: { unstaged: 'untracked' } }),
	makeEntry(6, 'junk.log', { git: { unstaged: 'ignored' } }),
	makeEntry(7, 'clash.txt', { git: { unstaged: 'conflicted' } }),
	makeEntry(8, 'src', { kind: 'directory', git: { inside: 3, conflictedInside: 0 } }),
	makeEntry(9, 'vendor', { kind: 'directory', git: { repository: true } }),
];

interface Options {
	repository?: boolean;
	settings?: Partial<Settings['general']>;
	ui?: Partial<Settings['ui']>;
	withGit?: boolean;
}

function mount(view: 'list' | 'grid', options: Options = {}) {
	const vfs = new FakeVfsClient();
	vfs.setFolder(FOLDER, entries());
	const git: FakeGitClient = createFakeGitClient(
		options.repository === false
			? []
			: [{ root: FOLDER, name: 'test', summary: cleanSummary({ staged: 1 }) }],
	);
	const settings: FakeSettings = createFakeSettingsClient({
		...DEFAULT_SETTINGS,
		general: { ...DEFAULT_SETTINGS.general, ...options.settings },
		ui: { ...DEFAULT_SETTINGS.ui, ...options.ui },
	});
	function Grid() {
		const state = useListingSession(useVfsClient(), FOLDER);
		return <GridView state={state} size={96} />;
	}
	render(
		<SettingsProvider client={settings}>
			<GitProvider client={options.withGit === false ? undefined : git}>
				<VfsClientProvider client={vfs}>
					{view === 'list' ? <ListView location={FOLDER} /> : <Grid />}
				</VfsClientProvider>
			</GitProvider>
		</SettingsProvider>,
	);
	return { git, settings, vfs };
}

const row = (name: string) =>
	screen.getAllByRole('option').find((option) => within(option).queryByText(name) !== null)!;

describe('the list in a working tree', () => {
	it('adds a Git column whose cells carry the letters and, for a screen reader, the words', async () => {
		mount('list');
		await screen.findByRole('button', { name: 'Git' });
		await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(9));
		const cell = (name: string) => row(name).querySelector('[data-column="git"]') as HTMLElement;
		expect(cell('edited.txt')).toHaveTextContent('M');
		expect(cell('edited.txt')).toHaveTextContent('Modified, not staged');
		expect(cell('added.txt')).toHaveTextContent('Added, staged');
		expect(cell('both.txt')).toHaveTextContent('Modified, staged; Modified, not staged');
		expect(cell('new.txt')).toHaveTextContent('?');
		expect(cell('new.txt')).toHaveTextContent('Untracked');
		expect(cell('junk.log')).toHaveTextContent('Ignored');
		expect(cell('clash.txt')).toHaveTextContent('Conflict');
		expect(cell('src')).toHaveTextContent('3 changed items inside');
		expect(cell('clean.txt')).toHaveTextContent('');
		// A repository with nothing changed says so in words only.
		expect(cell('vendor')).toHaveTextContent('Git repository');
		// What the colour says, the letters and words say too: the cell has text whatever its colour.
		expect(cell('edited.txt').querySelector('[data-change]')).toHaveAttribute(
			'data-change',
			'modified',
		);
		expect(row('junk.log')).toHaveAttribute('data-ignored');
	});

	it('has no Git column outside a working tree, without a Git service, or with Git switched off', async () => {
		for (const options of [
			{ repository: false },
			{ withGit: false },
			{ settings: { gitDecorations: false } },
		]) {
			const { git } = mount('list', options);
			await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(9));
			expect(screen.queryByRole('button', { name: 'Git' })).toBeNull();
			if ('settings' in options) expect(git.watched).toHaveLength(0);
			cleanup();
		}
	});

	it('sorts by status when the column heading is pressed', async () => {
		mount('list');
		await userEvent.click(await screen.findByRole('button', { name: 'Git' }));
		await waitFor(() => {
			const names = screen.getAllByRole('option').map((option) => option.textContent ?? '');
			// Folders first, then conflicts, edits, additions, new files, clean, ignored.
			expect(names.findIndex((n) => n.includes('src'))).toBeLessThan(
				names.findIndex((n) => n.includes('clash.txt')),
			);
		});
		const order = () =>
			screen
				.getAllByRole('option')
				.map((option) => /([a-z]+\.(?:txt|log))|src|vendor/.exec(option.textContent ?? '')![0]);
		await waitFor(() =>
			expect(order().slice(2)).toEqual([
				'clash.txt',
				'edited.txt',
				'both.txt',
				'added.txt',
				'new.txt',
				'clean.txt',
				'junk.log',
			]),
		);
	});

	it('offers to hide the column from the header menu, and brings it back', async () => {
		const { settings } = mount('list');
		const heading = await screen.findByRole('button', { name: 'Git' });
		fireEvent.contextMenu(heading);
		await userEvent.click(await screen.findByRole('menuitemcheckbox', { name: 'Git status' }));
		await waitFor(() => expect(settings.uiCalls.at(-1)).toEqual({ gitColumn: false }));
		await waitFor(() => expect(screen.queryByRole('button', { name: 'Git' })).toBeNull());
		// With the column gone the rows still carry no stale cell, and the menu is gone from the header
		// of a folder with no column to toggle only outside a working tree: here it stays available.
		fireEvent.contextMenu(screen.getByRole('group', { name: 'Sort the list' }));
		const item = await screen.findByRole('menuitemcheckbox', { name: 'Git status' });
		expect(item).toHaveAttribute('aria-checked', 'false');
		await userEvent.click(item);
		await waitFor(() => expect(settings.uiCalls.at(-1)).toEqual({ gitColumn: true }));
		await screen.findByRole('button', { name: 'Git' });
	});

	it('opens the header menu from the keyboard', async () => {
		mount('list');
		const heading = await screen.findByRole('button', { name: 'Git' });
		heading.focus();
		fireEvent.keyDown(heading, { key: 'F10', shiftKey: true });
		expect(await screen.findByRole('menu', { name: 'Columns' })).toBeInTheDocument();
	});

	it('has no header menu outside a working tree', async () => {
		mount('list', { repository: false });
		await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(9));
		fireEvent.contextMenu(screen.getByRole('group', { name: 'Sort the list' }));
		expect(screen.queryByRole('menu')).toBeNull();
	});
});

describe('the grid in a working tree', () => {
	it('draws a chip at the corner of a changed item, with the words for a screen reader', async () => {
		mount('grid');
		await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(9));
		const edited = screen
			.getAllByRole('option')
			.find((o) => o.textContent?.includes('edited.txt'))!;
		expect(within(edited).getByText('Modified, not staged')).toBeInTheDocument();
		expect(edited.querySelector('[data-variant="chip"]')).toHaveAttribute(
			'data-change',
			'modified',
		);
		const clean = screen.getAllByRole('option').find((o) => o.textContent?.includes('clean.txt'))!;
		expect(clean.querySelector('[data-variant="chip"]')).toBeNull();
		const junk = screen.getAllByRole('option').find((o) => o.textContent?.includes('junk.log'))!;
		expect(junk).toHaveAttribute('data-ignored');
	});
});

describe('the folder badge', () => {
	it('is the Portage set’s Git sticker on a folder that is a repository, and no other set draws one', () => {
		const markup = (props: Record<string, unknown>) => {
			const { container } = render(<FileIcon group="folder" {...props} />);
			const html = container.innerHTML;
			cleanup();
			return html;
		};
		expect(markup({ theme: 'portage', badge: 'git' })).not.toBe(markup({ theme: 'portage' }));
		expect(markup({ theme: 'waypoint', badge: 'git' })).toBe(markup({ theme: 'waypoint' }));
	});
});
