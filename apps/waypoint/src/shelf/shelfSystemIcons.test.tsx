// Verifies the System icon set on the Shelf and in the stack of a Shelf drag: a program is drawn from its own icon, named by the token Rust registered for its place
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { dismissNotice } from '../app/notices';
import { configureSystemIcons } from '../icons/systemIcons';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import {
	createFakeSystemIconsClient,
	type FakeSystemIcons,
} from '../services/fakeSystemIconsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { fileLocation, makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME, renderWorkspace } from '../test/workspaceHarness';

const root = document.documentElement;
let restoreLayout: () => void;
let fake: FakeSystemIcons;

beforeEach(() => {
	restoreLayout = stubLayout(280);
	localStorage.clear();
	document.elementsFromPoint = () => [];
	root.dataset.iconTheme = 'system';
	fake = createFakeSystemIconsClient();
	configureSystemIcons(fake);
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
	configureSystemIcons(null);
	delete root.dataset.iconTheme;
	// @ts-expect-error happy-dom has no hit test; the tests stand one in
	delete document.elementsFromPoint;
});

async function mount() {
	const tabs = new FakeTabsApi(FakeTabsStore.singleWindow(), 'main-1');
	await tabs.openTab(HOME);
	await tabs.addToShelf([
		fileLocation('/home/test/Setup.exe'),
		fileLocation('/home/test/notes.txt'),
	]);
	const tree = createTree();
	tree.setFolder(HOME, [makeEntry(1, 'Setup.exe'), makeEntry(2, 'notes.txt')]);
	await renderWorkspace(tree, tabs, undefined, { ops: createFakeOpsClient() });
	fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
	return screen.findByRole('complementary', { name: 'Shelf' });
}

const files = () => fake.probed.filter((url) => url.startsWith('fake://file/l'));

describe('the Shelf under the System icon set', () => {
	it('registers the places of the files it shows, asks for a program’s own icon by token, and asks for no other file by place', async () => {
		await mount();
		await waitFor(() => expect(files()).toHaveLength(1));
		const asked = fake.registered.flat().map((location) => location.uri);
		expect(asked).toEqual(['file:///home/test/Setup.exe']);
		expect(files()[0]).toMatch(/^fake:\/\/file\/l1\?size=16&scale=1&m=0$/);
	});

	it('draws the type’s icon for a program whose place Rust will not draw from', async () => {
		fake = createFakeSystemIconsClient({ token: () => null });
		configureSystemIcons(fake);
		await mount();
		await waitFor(() => expect(fake.probed.some((url) => url.includes('/ext/exe'))).toBe(true));
		expect(files()).toEqual([]);
	});

	it('draws the program’s own icon in the stack of a drag, from the token the row already has', async () => {
		const panel = await mount();
		await waitFor(() => expect(files()).toHaveLength(1));
		await act(async () => fake.settleUrl(files()[0]!, true));
		const row = [...panel.querySelectorAll('[role="treeitem"]')].find((item) =>
			item.textContent?.includes('Setup.exe'),
		)!;
		fireEvent.pointerDown(row, { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
		fireEvent.pointerMove(window, { pointerId: 1, clientX: 140, clientY: 100 });
		await waitFor(() => expect(document.querySelector('[data-drag-stack]')).not.toBeNull());
		await waitFor(() =>
			expect(
				document.querySelector('[data-drag-stack] svg[data-system] image')?.getAttribute('href'),
			).toBe(files()[0]),
		);
		// The same place is the same token, so the stack asked Rust for nothing more.
		expect(fake.registered).toHaveLength(1);
		fireEvent.pointerUp(window, { pointerId: 1, clientX: 140, clientY: 100 });
	});
});
