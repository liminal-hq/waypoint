// Verifies what a folder shows: its remembered choices over the window's, written through, and the cases where nothing is remembered
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import { describe, expect, it } from 'vitest';
import { createFakeFolderViewsClient } from '../services/fakeFolderViewsClient';
import { FakeVfsClient, fileLocation, syntheticEntries } from '../services/fakeVfsClient';
import { FolderViewController, folderViewKey } from './folderViewController';
import { createFolderViewsStore, type FolderViewsHandle } from './folderViewStore';
import { ListingManager } from './listingManager';
import { createViewStore } from './viewStore';

const A = fileLocation('/a');
const B = fileLocation('/b');
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

const bySize: SortSpec = { key: 'size', descending: true, directoriesFirst: true, groupBy: 'none' };
const byKind: SortSpec = {
	key: 'name',
	descending: false,
	directoriesFirst: true,
	groupBy: 'kind',
};

function tab(id: number, location = A): TabSnapshot {
	return {
		id,
		location,
		back: [],
		forward: [],
		pinned: false,
		colour: null,
		group: null,
		hints: { scrollTop: 0, focused: null },
	};
}

async function setup(
	remembered: Record<string, { mode: 'list' | 'grid' | null; sort: SortSpec | null }> = {},
) {
	const views = createFakeFolderViewsClient(remembered);
	const handle = createFolderViewsStore(views);
	await handle.ready;
	const viewStore = createViewStore();
	const controller = new FolderViewController(viewStore);
	const client = new FakeVfsClient();
	client.setFolder(A, syntheticEntries(20));
	client.setFolder(B, syntheticEntries(20));
	const manager = new ListingManager(client, {
		openOptions: (_inherited, location) => ({ sort: controller.sortFor(location) }),
		onSort: (sort, location) => controller.onSort(sort, location),
	});
	const configure = (
		activeKey: string | null,
		enabled = true,
		h: FolderViewsHandle | null = handle,
	) => controller.configure({ handle: h, enabled, activeKey });
	configure(A.uri);
	return { views, handle, viewStore, controller, manager, configure };
}

async function sortOf(manager: ListingManager, tabId: number) {
	const state = manager.stateFor(tabId);
	if (state?.status !== 'ready') throw new Error('not ready');
	return state.session.model;
}

describe('the key a folder is remembered under', () => {
	it('is the location’s uri, and none for what cannot remember', () => {
		expect(folderViewKey(A)).toBe(A.uri);
		expect(folderViewKey(null)).toBeNull();
		expect(folderViewKey(undefined)).toBeNull();
		expect(folderViewKey({ display: 'Overview', uri: 'overview:/' })).toBeNull();
		expect(folderViewKey({ display: 'Trash', uri: 'trash:///' })).toBeNull();
	});
});

describe('the sort a listing opens with', () => {
	it('is the folder’s own when it has one, and the window’s when it has not', async () => {
		const { manager } = await setup({ [A.uri]: { mode: null, sort: bySize } });
		manager.sync([tab(1, A), tab(2, B)], new Set([1, 2]));
		await settle();
		expect((await sortOf(manager, 1)).sort).toEqual(bySize);
		expect((await sortOf(manager, 2)).sort.key).toBe('name');
	});

	it('is the window’s for every folder while remembering is off', async () => {
		const { manager, configure } = await setup({ [A.uri]: { mode: null, sort: bySize } });
		configure(A.uri, false);
		manager.sync([tab(1, A)], new Set([1]));
		await settle();
		expect((await sortOf(manager, 1)).sort.key).toBe('name');
	});
});

describe('a sort changed in a folder', () => {
	it('is remembered by the folder and not by the window', async () => {
		const { manager, views, viewStore } = await setup();
		manager.sync([tab(1, A), tab(2, B)], new Set([1, 2]));
		await settle();
		await (await sortOf(manager, 1)).setSort(bySize);
		await settle();
		expect(views.view(A.uri)?.sort).toEqual(bySize);
		expect(views.view(B.uri)).toBeUndefined();
		expect(viewStore.getState().sort.key).toBe('name');
		expect(views.remembered).toEqual([{ key: A.uri, patch: { mode: null, sort: bySize } }]);
	});

	it('becomes the window’s while remembering is off, as it was before folders remembered', async () => {
		const { manager, views, viewStore, configure } = await setup();
		configure(A.uri, false);
		manager.sync([tab(1, A)], new Set([1]));
		await settle();
		await (await sortOf(manager, 1)).setSort(byKind);
		expect(viewStore.getState().sort).toEqual(byKind);
		expect(views.remembered).toEqual([]);
	});

	it('is not written again when it is what the folder already shows', async () => {
		const { manager, views } = await setup({ [A.uri]: { mode: null, sort: bySize } });
		manager.sync([tab(1, A)], new Set([1]));
		await settle();
		await (await sortOf(manager, 1)).setSort({ ...bySize });
		expect(views.remembered).toEqual([]);
	});
});

describe('reconciling the open listings', () => {
	it('gives each listing its folder’s sort when what the folders remember changes, without writing it back', async () => {
		const { manager, views, controller } = await setup();
		manager.sync([tab(1, A), tab(2, B)], new Set([1, 2]));
		await settle();
		views.change(A.uri, { mode: null, sort: byKind });
		controller.reconcile(manager);
		await settle();
		expect((await sortOf(manager, 1)).sort).toEqual(byKind);
		expect((await sortOf(manager, 2)).sort.groupBy).toBe('none');
		expect(views.remembered).toEqual([]);
	});

	it('leaves the folders alone while this window’s own writes are unanswered', async () => {
		const { manager, views, controller, handle } = await setup();
		manager.sync([tab(1, A)], new Set([1]));
		await settle();
		const hold = views.holdWrites();
		const write = handle.remember(A.uri, { sort: bySize });
		views.change(A.uri, { mode: null, sort: byKind });
		controller.reconcile(manager);
		await settle();
		expect((await sortOf(manager, 1)).sort.groupBy).toBe('none');
		hold.release();
		await write;
		controller.reconcile(manager);
		await settle();
		expect((await sortOf(manager, 1)).sort).toEqual(byKind);
	});

	it('puts the window’s sort back on every listing when remembering is turned off', async () => {
		const { manager, controller, configure } = await setup({
			[A.uri]: { mode: null, sort: bySize },
		});
		manager.sync([tab(1, A)], new Set([1]));
		await settle();
		expect((await sortOf(manager, 1)).sort).toEqual(bySize);
		configure(A.uri, false);
		controller.reconcile(manager);
		await settle();
		expect((await sortOf(manager, 1)).sort.key).toBe('name');
	});
});

describe('the view mode', () => {
	it('follows the folder in the active tab, and the window’s own mode shows where the folder has none', async () => {
		const { viewStore, controller, manager, configure } = await setup({
			[A.uri]: { mode: 'grid', sort: null },
		});
		controller.reconcile(manager);
		expect(viewStore.getState().mode).toBe('grid');
		configure(B.uri);
		controller.reconcile(manager);
		expect(viewStore.getState().mode).toBe('list');
		configure(null);
		viewStore.getState().setMode('grid');
		controller.reconcile(manager);
		// The Trash and Overview keep the mode they have.
		expect(viewStore.getState().mode).toBe('grid');
	});

	it('is remembered by the folder it was chosen in, and what the controller applies is not', async () => {
		const { viewStore, controller, manager, views, configure } = await setup({
			[B.uri]: { mode: 'grid', sort: null },
		});
		const stop = controller.followMode();
		configure(B.uri);
		controller.reconcile(manager);
		expect(views.remembered).toEqual([]);
		viewStore.getState().setMode('list');
		await settle();
		expect(views.remembered).toEqual([{ key: B.uri, patch: { mode: 'list', sort: null } }]);
		stop();
	});

	it('is the window’s while remembering is off, and that choice is what unremembered folders show later', async () => {
		const { viewStore, controller, manager, views, configure } = await setup({
			[A.uri]: { mode: 'grid', sort: null },
		});
		const stop = controller.followMode();
		configure(B.uri, false);
		viewStore.getState().setMode('grid');
		expect(views.remembered).toEqual([]);
		configure(B.uri, true);
		controller.reconcile(manager);
		// B remembers nothing, so it shows what the window chose while off.
		expect(viewStore.getState().mode).toBe('grid');
		configure(A.uri);
		controller.reconcile(manager);
		expect(viewStore.getState().mode).toBe('grid');
		stop();
	});
});

describe('resetting a folder', () => {
	it('asks Rust to forget it, once there is something to forget', async () => {
		const { controller, views } = await setup({ [A.uri]: { mode: 'grid', sort: null } });
		expect(controller.canReset(A.uri)).toBe(true);
		expect(controller.canReset(B.uri)).toBe(false);
		expect(controller.canReset(null)).toBe(false);
		await controller.resetActive();
		expect(views.resets).toEqual([A.uri]);
		expect(controller.canReset(A.uri)).toBe(false);
	});

	it('does nothing where there is no service or remembering is off', async () => {
		const { controller, views, configure } = await setup({ [A.uri]: { mode: 'grid', sort: null } });
		configure(A.uri, false);
		await controller.resetActive();
		expect(controller.canReset(A.uri)).toBe(false);
		configure(A.uri, true, null);
		await controller.resetActive();
		expect(views.resets).toEqual([]);
	});
});
