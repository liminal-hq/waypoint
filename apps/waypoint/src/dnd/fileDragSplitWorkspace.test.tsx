// Verifies dragging a Places item or a Folders-tree item (or a folder with Alt held) opens a new pane in the whole window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { dismissNotice } from '../app/notices';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { stubLayout } from '../test/browseHarness';
import { createTree, DOCS, HOME, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
let restoreRect: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
	localStorage.clear();
	document.elementsFromPoint = () => [];
	// The file area is measured from the document; give it a size.
	const original = HTMLElement.prototype.getBoundingClientRect;
	HTMLElement.prototype.getBoundingClientRect = function (this: HTMLElement) {
		return this.hasAttribute('data-pane-area')
			? ({
					left: 300,
					top: 100,
					right: 1200,
					bottom: 700,
					width: 900,
					height: 600,
					x: 300,
					y: 100,
				} as DOMRect)
			: original.call(this);
	};
	restoreRect = () => {
		HTMLElement.prototype.getBoundingClientRect = original;
	};
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
	restoreRect();
	// @ts-expect-error happy-dom has no hit test; the tests stand one in
	delete document.elementsFromPoint;
});

async function mount(paired = false) {
	const vfs = createTree();
	const tabs = new FakeTabsApi();
	await tabs.openTab(HOME);
	if (paired) {
		const second = await tabs.openTab(DOCS);
		const first = (await tabs.getSnapshot()).tabs[0]!.id;
		await tabs.joinPair([first, second], 'sideBySide');
		await tabs.activateTab(first);
	}
	const places = new FakePlacesClient({ places: fakePlaces('/home/test') });
	await renderWorkspace(vfs, tabs, places, { sidebar: true, ops: createFakeOpsClient() });
	return { tabs };
}

const placeButton = () =>
	within(
		within(screen.getByRole('navigation', { name: 'Sidebar' })).getByRole('group', {
			name: 'Places',
		}),
	).getByRole('button', { name: /^Documents/ });
const moveTo = (x: number, y: number) =>
	fireEvent.pointerMove(window, { pointerId: 1, clientX: x, clientY: y });
const pill = () => document.querySelector('[data-drag-pill]')?.textContent ?? null;
const zones = () => document.querySelector('[data-split-zones]');

describe('dragging a place from the sidebar', () => {
	it('shows the split zones over the file area and opens the folder in a new pane on release', async () => {
		const { tabs } = await mount();
		const place = await waitFor(() => placeButton());
		fireEvent.pointerDown(place, { pointerId: 1, button: 0, clientX: 50, clientY: 200 });
		moveTo(70, 200);
		expect(pill()).toContain('Dragging Documents');
		expect(zones()).toBeNull();

		moveTo(350, 400);
		await waitFor(() => expect(zones()).toHaveAttribute('data-edge', 'left'));
		expect(pill()).toContain('Open Documents in a new left pane');

		fireEvent.pointerUp(window, { pointerId: 1, clientX: 350, clientY: 400 });
		await waitFor(async () => expect((await tabs.getSnapshot()).pairs).toHaveLength(1));
		const snapshot = await tabs.getSnapshot();
		const pair = snapshot.pairs[0]!;
		expect(pair.layout).toBe('sideBySide');
		const opened = snapshot.tabs.find((tab) => tab.location.uri.endsWith('/Documents'))!;
		expect(opened).toBeDefined();
		// A drop on the left puts the new pane first.
		expect(pair.panes[0]).toBe(opened.id);
		expect(snapshot.active).toBe(opened.id);
		expect(zones()).toBeNull();
	});

	it('is not a click: the press that became a drag does not navigate', async () => {
		const { tabs } = await mount();
		const place = await waitFor(() => placeButton());
		fireEvent.pointerDown(place, { pointerId: 1, button: 0, clientX: 50, clientY: 200 });
		moveTo(70, 200);
		fireEvent.pointerUp(window, { pointerId: 1, clientX: 70, clientY: 200 });
		fireEvent.click(place);
		await Promise.resolve();
		const snapshot = await tabs.getSnapshot();
		expect(snapshot.tabs).toHaveLength(1);
		expect(snapshot.tabs[0]!.location.uri).toBe(HOME.uri);
	});

	it('refuses, and says why, when the tab on show is already split', async () => {
		const { tabs } = await mount(true);
		const place = await waitFor(() => placeButton());
		fireEvent.pointerDown(place, { pointerId: 1, button: 0, clientX: 50, clientY: 200 });
		moveTo(70, 200);
		moveTo(350, 400);
		await waitFor(() => expect(pill()).toContain('Not allowed: this tab is already split'));
		expect(zones()).toBeNull();
		fireEvent.pointerUp(window, { pointerId: 1, clientX: 350, clientY: 400 });
		expect((await tabs.getSnapshot()).pairs).toHaveLength(1);
		expect((await tabs.getSnapshot()).tabs).toHaveLength(2);
	});
});

describe('dragging a folder from the file view', () => {
	const row = (name: string) =>
		within(screen.getByRole('listbox', { name: 'Files' }))
			.getAllByRole('option')
			.find((option) => option.textContent?.includes(name))!;

	it('copies or moves into the pane as always, and opens a new pane with Alt held', async () => {
		const { tabs } = await mount();
		await waitFor(() => expect(row('docs')).toBeDefined());
		fireEvent.pointerDown(row('docs'), { pointerId: 1, button: 0, clientX: 400, clientY: 200 });
		moveTo(420, 200);
		expect(zones()).toBeNull();
		// Without Alt a drop over the pane is a file drop: no zones.
		moveTo(1100, 400);
		expect(zones()).toBeNull();
		fireEvent.pointerMove(window, { pointerId: 1, clientX: 1100, clientY: 400, altKey: true });
		await waitFor(() => expect(zones()).toHaveAttribute('data-edge', 'right'));
		expect(pill()).toContain('Open docs in a new right pane');
		fireEvent.pointerUp(window, { pointerId: 1, clientX: 1100, clientY: 400, altKey: true });
		await waitFor(async () => expect((await tabs.getSnapshot()).pairs).toHaveLength(1));
		const snapshot = await tabs.getSnapshot();
		const opened = snapshot.tabs.find((tab) => tab.location.uri === DOCS.uri)!;
		expect(snapshot.pairs[0]!.panes[1]).toBe(opened.id);
	});
});
