// Verifies the Inspector's Git tab: present only in a working tree, lazy, read only, and in words
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { dismissNotice } from '../app/notices';
import { cleanSummary, createFakeGitClient } from '../services/fakeGitClient';
import { fakeDetails } from '../services/fakeDetailsClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { fileLocation, makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME, renderWorkspace } from '../test/workspaceHarness';
import { LazyDetails } from './inspectorHarness';
import { HEAVY_DELAY_MS } from './inspectorModel';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
});

const option = (name: string) => screen.findByRole('option', { name: new RegExp(`^${name}`) });
const panel = () => screen.findByRole('complementary', { name: 'Inspector' });

const NOTES = fileLocation('/home/test/notes.txt');

function details() {
	return new LazyDetails({
		3: { details: fakeDetails({ name: 'notes.txt', owner: 'scott', unavailable: [] }) },
	});
}

function repository(summary = {}) {
	const git = createFakeGitClient([{ root: HOME, name: 'test', summary: cleanSummary(summary) }]);
	git.pathInfos.set(NOTES.uri, {
		commits: [
			{
				id: 'a'.repeat(40),
				short: 'aaaaaaaa',
				summary: 'Fix the notes',
				author: 'Ada',
				timeMs: 1_767_398_400_000,
			},
		],
		truncated: false,
		diff: { files: 1, added: 3, removed: 1, binary: 0, partial: false },
	});
	git.pathInfos.set(HOME.uri, {
		commits: [],
		truncated: true,
		diff: { files: 0, added: 0, removed: 0, binary: 0, partial: false },
	});
	return git;
}

async function openTab(git = repository()) {
	await renderWorkspace(createTree(), undefined, undefined, { details: details(), git });
	await option('notes.txt');
	fireEvent.keyDown(window, { key: 'F11' });
	const inspector = await panel();
	const tab = await within(inspector).findByRole('tab', { name: 'Git' });
	return { git, inspector, tab };
}

describe('the Git tab', () => {
	it('is the third tab in a working tree and is not there outside one, without a service or with Git off', async () => {
		await renderWorkspace(createTree(), undefined, undefined, {
			details: details(),
			git: createFakeGitClient(),
		});
		await option('notes.txt');
		fireEvent.keyDown(window, { key: 'F11' });
		const outside = await panel();
		expect(
			within(outside)
				.getAllByRole('tab')
				.map((t) => t.textContent),
		).toEqual(['Preview', 'Properties']);
		cleanup();
		const { inspector } = await openTab();
		expect(
			within(inspector)
				.getAllByRole('tab')
				.map((t) => t.textContent),
		).toEqual(['Preview', 'Properties', 'Git']);
	});

	it('is chosen like the others, with the keys, and names its panel', async () => {
		const { inspector, tab } = await openTab();
		fireEvent.click(within(inspector).getByRole('tab', { name: 'Properties' }));
		const properties = within(inspector).getByRole('tab', { name: 'Properties' });
		properties.focus();
		fireEvent.keyDown(properties.parentElement!, { key: 'ArrowRight' });
		expect(tab).toHaveAttribute('aria-selected', 'true');
		expect(tab).toHaveFocus();
		const body = within(inspector).getByRole('tabpanel', { name: 'Git' });
		expect(body).toBeVisible();
		// End from the last tab stays on it, Home goes back to Preview.
		fireEvent.keyDown(tab.parentElement!, { key: 'Home' });
		expect(within(inspector).getByRole('tab', { name: 'Preview' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
	});

	it('shows the repository, the branch and the counts, with the selected item’s history', async () => {
		const { inspector, tab } = await openTab(
			repository({ upstream: 'origin/main', ahead: 1, behind: 0, unstaged: 2, untracked: 1 }),
		);
		fireEvent.click(tab);
		const body = within(inspector).getByRole('tabpanel', { name: 'Git' });
		expect(body).toHaveTextContent('Git repository test');
		expect(body).toHaveTextContent('origin/main');
		expect(body).toHaveTextContent('2 not staged, 1 untracked');
		fireEvent.click(await option('notes.txt'));
		await waitFor(() => expect(body).toHaveTextContent('Fix the notes'), { timeout: 2000 });
		expect(body).toHaveTextContent('aaaaaaaa');
		expect(body).toHaveTextContent('Ada');
		expect(body).toHaveTextContent('1 file changed');
		expect(body).toHaveTextContent('+3 −1 lines');
		expect(body).toHaveTextContent('No changes');
	});

	it('says the selected item’s status in words, from its mark', async () => {
		const git = repository();
		const vfs = createTree();
		vfs.setFolder(HOME, [
			makeEntry(1, 'docs', { kind: 'directory' }),
			makeEntry(2, 'music', { kind: 'directory' }),
			makeEntry(3, 'notes.txt', { git: { staged: 'modified', unstaged: 'modified' } }),
			makeEntry(4, 'photo.jpg'),
		]);
		await renderWorkspace(vfs, undefined, undefined, { details: details(), git });
		fireEvent.click(await option('notes.txt'));
		fireEvent.keyDown(window, { key: 'F11' });
		const inspector = await panel();
		fireEvent.click(await within(inspector).findByRole('tab', { name: 'Git' }));
		const body = within(inspector).getByRole('tabpanel', { name: 'Git' });
		expect(body).toHaveTextContent('Modified, staged; Modified, not staged');
		fireEvent.click(await option('photo.jpg'));
		await waitFor(() => expect(body).not.toHaveTextContent('Modified, staged'));
	});

	it('reads history only once the selection has held still, and not for a held arrow key', async () => {
		const { git, tab } = await openTab();
		fireEvent.click(tab);
		const list = screen.getByRole('listbox');
		act(() => list.focus());
		const before = git.pathInfoAsked.length;
		for (let i = 0; i < 4; i++) fireEvent.keyDown(list, { key: 'ArrowDown' });
		expect(git.pathInfoAsked.length).toBe(before);
		await new Promise((resolve) => setTimeout(resolve, HEAVY_DELAY_MS + 100));
		await waitFor(() => expect(git.pathInfoAsked.length).toBeGreaterThan(before));
		// One question for where the selection stopped, not one for each row passed.
		expect(git.pathInfoAsked.length - before).toBeLessThanOrEqual(2);
	});

	it('says an old folder may have older history, and what it could not read', async () => {
		const git = repository();
		const { inspector, tab } = await openTab(git);
		fireEvent.click(tab);
		const body = within(inspector).getByRole('tabpanel', { name: 'Git' });
		await waitFor(() => expect(body).toHaveTextContent('Older commits may exist'), {
			timeout: 2000,
		});
		expect(body).toHaveTextContent('No commit has changed this yet.');
		git.pathInfos.delete(HOME.uri);
	});

	it('falls back to Preview when Git is switched off, and offers no write action', async () => {
		const settings = createFakeSettingsClient();
		const git = repository();
		await renderWorkspace(createTree(), undefined, undefined, {
			details: details(),
			git,
			settings,
		});
		await option('notes.txt');
		fireEvent.keyDown(window, { key: 'F11' });
		const inspector = await panel();
		fireEvent.click(await within(inspector).findByRole('tab', { name: 'Git' }));
		const body = within(inspector).getByRole('tabpanel', { name: 'Git' });
		// Read only: the tab holds no button, so nothing in it stages, commits or discards.
		expect(within(body).queryAllByRole('button')).toHaveLength(0);
		act(() => {
			settings.change({
				...settings.current().settings,
				general: { ...settings.current().settings.general, gitDecorations: false },
			});
		});
		await waitFor(() => expect(within(inspector).queryByRole('tab', { name: 'Git' })).toBeNull());
		expect(within(inspector).getByRole('tab', { name: 'Preview' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
	});
});
