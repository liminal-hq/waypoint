// Verifies the in-memory GitClient keeps the plugin's contract, so a view test that uses it means something
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { cleanSummary, createFakeGitClient } from './fakeGitClient';
import { fileLocation } from './fakeVfsClient';

const ROOT = fileLocation('/home/me/repo');
const repository = () => ({ root: ROOT, name: 'repo', summary: cleanSummary() });

describe('the fake Git client', () => {
	it('finds the repository a folder is in, by the folder starting with its root, and numbers its watches', async () => {
		const git = createFakeGitClient([repository()]);
		const first = await git.watch(fileLocation('/home/me/repo/src'));
		const second = await git.watch(ROOT);
		expect(first).toMatchObject({ id: 1, name: 'repo', revision: 1, root: ROOT });
		expect(second?.id).toBe(2);
		expect(await git.watch(fileLocation('/home/me/repo-two'))).toBeNull();
		expect(await git.watch(fileLocation('/elsewhere'))).toBeNull();
		expect(git.watched).toHaveLength(4);
		expect(git.watching.size).toBe(2);
	});

	it('stops a watch, and tells every watch of a repository when it changes, with rising revisions', async () => {
		const git = createFakeGitClient([repository()]);
		const heard = vi.fn();
		git.onChanged(heard);
		const a = (await git.watch(ROOT))!;
		const b = (await git.watch(fileLocation('/home/me/repo/src')))!;
		git.change(ROOT.uri, cleanSummary({ head: 'feature' }));
		expect(heard).toHaveBeenCalledTimes(2);
		expect(heard.mock.calls.map(([event]) => [event.id, event.revision])).toEqual([
			[a.id, 2],
			[b.id, 2],
		]);
		await git.unwatch(a.id);
		heard.mockClear();
		git.change(ROOT.uri, cleanSummary({ head: 'again' }));
		expect(heard).toHaveBeenCalledTimes(1);
		expect((await git.watch(ROOT))?.summary?.head).toBe('again');
	});

	it('answers badges for the locations asked about and can hold a reply', async () => {
		const git = createFakeGitClient([], [{ uri: ROOT.uri, changed: 2, conflicted: 0 }]);
		expect(await git.badges([ROOT, fileLocation('/other')])).toEqual([
			{ uri: ROOT.uri, changed: 2, conflicted: 0 },
		]);
		const held = createFakeGitClient([repository()]);
		const hold = held.holdWatch();
		let answered = false;
		void held.watch(ROOT).then(() => (answered = true));
		await new Promise((resolve) => setTimeout(resolve, 5));
		expect(answered).toBe(false);
		hold.release();
		await new Promise((resolve) => setTimeout(resolve, 5));
		expect(answered).toBe(true);
	});
});
