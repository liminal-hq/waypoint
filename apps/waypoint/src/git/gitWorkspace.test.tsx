// Verifies Git across the window: the branch in the status bar, the sidebar's dots, the sort command and the Settings switch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createFakeGitClient, cleanSummary } from '../services/fakeGitClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { fileLocation, makeEntry } from '../services/fakeVfsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, DOCS, HOME, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

const bar = () => screen.getByRole('group', { name: 'Status bar' });
const branch = () => within(bar()).queryByRole('group', { name: 'Git branch' });
const option = (name: string) => screen.findByRole('option', { name: new RegExp(`^${name}`) });

function repository(overrides = {}) {
	return createFakeGitClient([{ root: HOME, name: 'test', summary: cleanSummary(overrides) }]);
}

describe('the branch in the status bar', () => {
	it('shows the branch of the working tree the folder is in, and nothing outside one', async () => {
		const git = repository();
		await renderWorkspace(undefined, undefined, undefined, { git });
		await option('docs');
		await waitFor(() => expect(branch()).not.toBeNull());
		expect(branch()).toHaveTextContent('main');
		// Not the tooltip alone: the words are in the item for assistive technology.
		expect(branch()).toHaveTextContent('Git repository test, on branch main. No changes');
		cleanup();
		await renderWorkspace(undefined, undefined, undefined, { git: createFakeGitClient() });
		await option('docs');
		expect(branch()).toBeNull();
	});

	it('shows the distance from the upstream and a dot when something changed, and follows the plugin', async () => {
		const git = repository({ upstream: 'origin/main', ahead: 2, behind: 1 });
		await renderWorkspace(undefined, undefined, undefined, { git });
		await waitFor(() => expect(branch()).not.toBeNull());
		expect(branch()).toHaveTextContent('main ↑2 ↓1');
		expect(branch()!.querySelector('[data-dirty]')).toBeNull();
		act(() => {
			git.change(
				HOME.uri,
				cleanSummary({ head: 'feature', unstaged: 2, untracked: 1, operation: 'rebase' }),
			);
		});
		await waitFor(() => expect(branch()).toHaveTextContent('feature'));
		expect(branch()).toHaveAttribute('data-dirty');
		expect(branch()).toHaveAttribute('data-operation', 'rebase');
		expect(branch()).toHaveTextContent('A rebase is in progress');
		expect(branch()).toHaveTextContent('2 not staged, 1 untracked');
		expect(branch()).toHaveAttribute('title', expect.stringContaining('Git repository test'));
	});

	it('says a detached head and a fresh repository in words', async () => {
		const git = repository({ headKind: 'detached', head: 'a1b2c3d4' });
		await renderWorkspace(undefined, undefined, undefined, { git });
		await waitFor(() => expect(branch()).toHaveTextContent('Detached at a1b2c3d4'));
		expect(branch()).toHaveTextContent('detached at commit a1b2c3d4');
	});

	it('is not a live region: a change of branch is not announced on its own', async () => {
		const git = repository();
		await renderWorkspace(undefined, undefined, undefined, { git });
		await waitFor(() => expect(branch()).not.toBeNull());
		expect(branch()!.closest('[aria-live]')).toBeNull();
		expect(branch()).not.toHaveAttribute('aria-live');
	});

	it('stays while the folder changes inside the working tree, and lets go of a folder it left', async () => {
		const git = repository({ head: 'trunk' });
		await renderWorkspace(undefined, undefined, undefined, { git });
		await waitFor(() => expect(branch()).toHaveTextContent('trunk'));
		fireEvent.doubleClick(await option('docs'));
		await option('report.pdf');
		await waitFor(() => expect(branch()).toHaveTextContent('trunk'));
		// The folder it left is no longer watched; the one it is in, and the tab's own, are.
		await waitFor(() => expect([...git.watching.values()].map((l) => l.uri)).toContain(DOCS.uri));
		expect([...git.watching.values()].map((l) => l.uri)).not.toContain(HOME.uri);
	});
});

describe('the Settings switch', () => {
	it('turns the branch off and watches nothing', async () => {
		const git = repository();
		const settings = createFakeSettingsClient({
			...DEFAULT_SETTINGS,
			general: { ...DEFAULT_SETTINGS.general, gitDecorations: false },
		});
		await renderWorkspace(undefined, undefined, undefined, { git, settings });
		await option('docs');
		expect(branch()).toBeNull();
		expect(git.watched).toHaveLength(0);
	});

	it('stops and starts as the switch changes', async () => {
		const git = repository();
		const settings = createFakeSettingsClient();
		await renderWorkspace(undefined, undefined, undefined, { git, settings });
		await waitFor(() => expect(branch()).not.toBeNull());
		act(() => {
			settings.change({
				...settings.current().settings,
				general: { ...settings.current().settings.general, gitDecorations: false },
			});
		});
		await waitFor(() => expect(branch()).toBeNull());
		await waitFor(() => expect(git.watching.size).toBe(0));
		act(() => {
			settings.change({
				...settings.current().settings,
				general: { ...settings.current().settings.general, gitDecorations: true },
			});
		});
		await waitFor(() => expect(branch()).not.toBeNull());
	});
});

describe('the sidebar', () => {
	it('puts a dot beside a favourite or a place with changes inside, and says how many', async () => {
		const project = fileLocation('/srv/projects');
		const git = createFakeGitClient(
			[],
			[
				{ uri: project.uri, changed: 3, conflicted: 0 },
				{ uri: HOME.uri, changed: 1, conflicted: 1 },
			],
		);
		const client = createTree();
		client.setFolder(project, [makeEntry(1, 'code.ts')]);
		const places = new FakePlacesClient({
			places: fakePlaces('/home/test'),
			favourites: [{ label: 'Projects', location: project }],
		});
		await renderWorkspace(client, undefined, places, { sidebar: true, git });
		const sidebar = await screen.findByRole('navigation', { name: 'Sidebar' });
		const favourite = await within(sidebar).findByRole('button', { name: /Projects/ });
		await waitFor(() => expect(favourite).toHaveTextContent('3 changed items inside'));
		const home = within(sidebar).getByRole('button', { name: /^Home/ });
		await waitFor(() => expect(home).toHaveTextContent('1 changed item inside'));
		expect(home).toHaveTextContent('1 in conflict');
		expect(within(sidebar).getByRole('button', { name: /^Music/ })).not.toHaveTextContent(
			'changed',
		);
	});

	it('shows no dots with Git switched off', async () => {
		const project = fileLocation('/srv/projects');
		const git = createFakeGitClient([], [{ uri: project.uri, changed: 3, conflicted: 0 }]);
		const client = createTree();
		client.setFolder(project, [makeEntry(1, 'code.ts')]);
		const places = new FakePlacesClient({
			places: fakePlaces('/home/test'),
			favourites: [{ label: 'Projects', location: project }],
		});
		const settings = createFakeSettingsClient({
			...DEFAULT_SETTINGS,
			general: { ...DEFAULT_SETTINGS.general, gitDecorations: false },
		});
		await renderWorkspace(client, undefined, places, { sidebar: true, git, settings });
		const sidebar = await screen.findByRole('navigation', { name: 'Sidebar' });
		const favourite = await within(sidebar).findByRole('button', { name: /Projects/ });
		await new Promise((resolve) => setTimeout(resolve, 30));
		expect(favourite).not.toHaveTextContent('changed');
	});
});
