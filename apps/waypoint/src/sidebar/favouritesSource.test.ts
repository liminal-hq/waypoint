// Verifies a workspace's favourites source edits the newest list, one edit after another
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { fileLocation } from '../services/fakeVfsClient';
import { workspaceSource } from './favouritesSource';

async function setup() {
	const api = new FakeTabsApi(new FakeTabsStore(), 'main-1');
	const first = await api.openTab(fileLocation('/home/test/a'));
	const group = await api.createGroup([first], 'Work');
	const id = await api.saveGroupAsWorkspace(group, 'Site');
	const read = async () => (await api.getSnapshot()).workspaces.find((w) => w.id === id)!;
	return { api, id, read };
}

const A = fileLocation('/home/test/a');
const B = fileLocation('/home/test/b');
const C = fileLocation('/home/test/c');

describe('workspaceSource', () => {
	it('keeps both of two quick adds, though the source was built before either', async () => {
		const { api, read } = await setup();
		const source = workspaceSource(api, await read());
		await Promise.all([source.add(B), source.add(C)]);
		expect((await read()).locations.map((location) => location.uri)).toEqual(
			[A, B, C].map((l) => l.uri),
		);
	});

	it('applies an edit made from a stale render on top of the newest list', async () => {
		const { api, id, read } = await setup();
		const stale = workspaceSource(api, await read());
		await api.setWorkspaceLocations(id, [A, B]);
		await stale.remove(A);
		expect((await read()).locations.map((location) => location.uri)).toEqual([B.uri]);
	});

	it('moves within the newest list and reports its length', async () => {
		const { api, id, read } = await setup();
		await api.setWorkspaceLocations(id, [A, B, C]);
		const source = workspaceSource(api, await read());
		await expect(source.move(C, 0)).resolves.toBe(3);
		expect((await read()).locations.map((location) => location.uri)).toEqual(
			[C, A, B].map((l) => l.uri),
		);
	});

	it('does nothing for a workspace that has gone', async () => {
		const { api, id, read } = await setup();
		const source = workspaceSource(api, await read());
		await api.deleteWorkspace(id);
		await expect(source.add(B)).resolves.toBeUndefined();
	});
});
