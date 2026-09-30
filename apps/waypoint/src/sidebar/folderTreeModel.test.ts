// Verifies the Folders tree loads children lazily through folders-only listings and closes them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import { FolderTreeModel, MAX_CHILDREN } from './folderTreeModel';

const ROOT = fileLocation('/r');
const dir = (id: number, name: string) => makeEntry(id, name, { kind: 'directory' });

function setup() {
	const client = new FakeVfsClient();
	client.setFolder(ROOT, [dir(1, 'b'), dir(2, 'a'), makeEntry(3, 'file.txt'), dir(4, '.hidden')]);
	client.setFolder(fileLocation('/r/a'), [dir(1, 'inner')]);
	const model = new FolderTreeModel(client);
	return { client, model };
}
const names = (model: FolderTreeModel, uri: string) => {
	const state = model.childrenOf(uri);
	return state?.status === 'ready' ? state.children.map((child) => child.name) : state?.status;
};

let model: FolderTreeModel | undefined;
afterEach(() => model?.dispose());

describe('FolderTreeModel', () => {
	it('loads nothing until a node is wanted, then only its folders, in listing order', async () => {
		const made = setup();
		model = made.model;
		expect(made.client.openCount).toBe(0);
		model.sync([ROOT], false);
		expect(model.childrenOf(ROOT.uri)).toEqual({ status: 'loading' });
		await waitFor(() => expect(names(model!, ROOT.uri)).toEqual(['a', 'b']));
		expect(made.client.openCount).toBe(1);
		const state = model.childrenOf(ROOT.uri);
		expect(state).toMatchObject({ total: 2 });
		expect(state?.status === 'ready' && state.children[0]!.location).toEqual(fileLocation('/r/a'));
	});

	it('shows hidden folders when asked to, including for listings already open', async () => {
		const made = setup();
		model = made.model;
		model.sync([ROOT], true);
		await waitFor(() => expect(names(model!, ROOT.uri)).toEqual(['.hidden', 'a', 'b']));
		model.sync([ROOT], false);
		await waitFor(() => expect(names(model!, ROOT.uri)).toEqual(['a', 'b']));
		expect(made.client.openCount).toBe(1);
	});

	it('closes the listing of a node that is no longer wanted, and on dispose', async () => {
		const made = setup();
		model = made.model;
		const inner = fileLocation('/r/a');
		model.sync([ROOT, inner], false);
		await waitFor(() => expect(names(model!, inner.uri)).toEqual(['inner']));
		expect(made.client.openCount).toBe(2);
		model.sync([ROOT], false);
		expect(model.childrenOf(inner.uri)).toBeUndefined();
		await waitFor(() => expect(made.client.openCount).toBe(1));
		model.dispose();
		await waitFor(() => expect(made.client.openCount).toBe(0));
	});

	it('closes a listing that finished opening after its node stopped being wanted', async () => {
		const client = new FakeVfsClient({ latencyMs: 10 });
		client.setFolder(ROOT, [dir(1, 'a')]);
		model = new FolderTreeModel(client);
		model.sync([ROOT], false);
		model.sync([], false);
		await waitFor(() => expect(client.openCount).toBe(0));
		await new Promise((resolve) => setTimeout(resolve, 40));
		expect(client.openCount).toBe(0);
		expect(model.childrenOf(ROOT.uri)).toBeUndefined();
	});

	it('follows folders created and removed underneath while the node stays open', async () => {
		const made = setup();
		model = made.model;
		model.sync([ROOT], false);
		await waitFor(() => expect(names(model!, ROOT.uri)).toEqual(['a', 'b']));
		made.client.setFolder(ROOT, [dir(1, 'b'), dir(2, 'a'), dir(5, 'c')]);
		await waitFor(() => expect(names(model!, ROOT.uri)).toEqual(['a', 'b', 'c']));
		made.client.setFolder(ROOT, [dir(2, 'a')]);
		await waitFor(() => expect(names(model!, ROOT.uri)).toEqual(['a']));
	});

	it('reports a folder that cannot be listed as an error state, and tells subscribers', async () => {
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		const client = new FakeVfsClient();
		model = new FolderTreeModel(client);
		const listener = vi.fn();
		model.subscribe(listener);
		model.sync([fileLocation('/gone')], false);
		await waitFor(() =>
			expect(model!.childrenOf(fileLocation('/gone').uri)).toEqual({ status: 'error' }),
		);
		expect(listener).toHaveBeenCalled();
		vi.restoreAllMocks();
	});

	it('reads at most MAX_CHILDREN children and reports the total', async () => {
		const client = new FakeVfsClient();
		client.setFolder(
			ROOT,
			Array.from({ length: MAX_CHILDREN + 5 }, (_, index) => dir(index + 1, `d${index}`)),
		);
		model = new FolderTreeModel(client);
		model.sync([ROOT], false);
		await waitFor(() => expect(model!.childrenOf(ROOT.uri)?.status).toBe('ready'));
		const state = model.childrenOf(ROOT.uri);
		expect(state?.status === 'ready' && [state.children.length, state.total]).toEqual([
			MAX_CHILDREN,
			MAX_CHILDREN + 5,
		]);
	});
});
