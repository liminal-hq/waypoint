// Verifies the tab commands decide from the live session, not from the snapshot they were built with
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { fileLocation } from '../services/fakeVfsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { createTabActions } from './tabActions';

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('close', () => {
	it('leaves one Home tab when the last tab is closed twice in quick succession', async () => {
		const api = new FakeTabsApi();
		const home = fileLocation('/home/test');
		const id = await api.openTab(fileLocation('/home/test/docs'));
		const stale = await api.getSnapshot();
		const actions = createTabActions(api, stale, home);

		actions.close(id);
		actions.close(id);
		await settle();

		const after = await api.getSnapshot();
		expect(after.tabs).toHaveLength(1);
		expect(after.tabs[0]!.location.uri).toBe(home.uri);
	});

	it('closes two of three tabs without opening a replacement', async () => {
		const api = new FakeTabsApi();
		const home = fileLocation('/home/test');
		const a = await api.openTab(home);
		const b = await api.openTab(home);
		await api.openTab(home);
		const actions = createTabActions(api, await api.getSnapshot(), home);
		actions.close(a);
		actions.close(b);
		await settle();
		expect((await api.getSnapshot()).tabs).toHaveLength(1);
	});
});
