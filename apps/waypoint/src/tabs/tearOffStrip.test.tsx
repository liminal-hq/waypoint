// Verifies the new-window phase over the real strip and file area: regions, the card and pill, a release and the pane grip
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTearoffClient, reportAt } from '../services/fakeTearoffClient';
import { stubLayout } from '../test/browseHarness';
import { DOCS, MUSIC, renderWorkspace } from '../test/workspaceHarness';
import { endRename } from './groupActions';
import { REGION_DELAY_MS } from './useTearoff';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	endRename();
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
	document.documentElement.removeAttribute('style');
});

type Harness = Awaited<ReturnType<typeof renderWorkspace>>;

const tabs = () => screen.getAllByRole('tab');
const slot = (index: number) => tabs()[index]!.parentElement!;
const pills = () => Array.from(document.querySelectorAll('[data-drag-pill]'));
const cards = () => Array.from(document.querySelectorAll('[data-tear-off-card]'));
const status = () =>
	screen
		.getAllByRole('status')
		.map((node) => node.textContent)
		.join('|');

/** Tabs 100 wide, a strip 0..30 tall and 800 wide, a file area 200..600 tall. */
function stubGeometry() {
	return vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
		this: HTMLElement,
	) {
		const rect = (left: number, top: number, right: number, bottom: number) =>
			({ left, top, right, bottom, width: right - left, height: bottom - top }) as DOMRect;
		const index = this.getAttribute('data-index');
		if (index !== null) return rect(Number(index) * 100, 0, Number(index) * 100 + 100, 30);
		if (this.hasAttribute('data-chip')) return rect(-100, 0, -10, 30);
		if (this.hasAttribute('data-strip')) return rect(0, 0, 800, 30);
		if (this.hasAttribute('data-pane-area')) return rect(0, 200, 1000, 600);
		return rect(0, 0, 100, 30);
	});
}

async function openTwo(h: Harness) {
	await h.tabs.openTab(DOCS);
	await h.tabs.openTab(MUSIC);
	await waitFor(() => expect(tabs()).toHaveLength(3));
}

describe('the plugin at start-up', () => {
	it('is asked for its features once, and registers no regions where it cannot hit-test', async () => {
		stubGeometry();
		const client = new FakeTearoffClient({ ghost: true, cursorFollow: true });
		const h = await renderWorkspace(undefined, undefined, undefined, { tearoff: client });
		await openTwo(h);
		expect(client.statusCalls).toBe(1);
		expect(client.calls).not.toContain('setDropRegions');
	});
});

describe('the drop regions', () => {
	it('register the strip and each tab, follow the tabs, and clear when the window closes', async () => {
		stubGeometry();
		const client = new FakeTearoffClient({ hitTest: true });
		const h = await renderWorkspace(undefined, undefined, undefined, { tearoff: client });
		await waitFor(() => expect(client.regions.map((region) => region.id)).toContain('strip'));
		expect(client.regions.map((region) => region.id)).toEqual(['strip', 'slot:0', 'slot:1']);
		expect(client.regions[0]).toEqual({ id: 'strip', x: 0, y: 0, width: 800, height: 30 });

		await openTwo(h);
		await waitFor(() =>
			expect(client.regions.map((region) => region.id)).toEqual([
				'strip',
				'slot:0',
				'slot:1',
				'slot:1',
				'slot:2',
				'slot:2',
				'slot:3',
			]),
		);
		// A burst of resizes is one update.
		const before = client.calls.filter((call) => call === 'setDropRegions').length;
		for (let i = 0; i < 5; i++) fireEvent(window, new Event('resize'));
		await new Promise((resolve) => setTimeout(resolve, REGION_DELAY_MS * 2));
		expect(client.calls.filter((call) => call === 'setDropRegions').length).toBeLessThanOrEqual(
			before + 1,
		);

		h.unmount();
		expect(client.regions).toEqual([]);
	});
});

describe('a drag out of the strip', () => {
	it('shows the card and the pill, and opens a window on release', async () => {
		stubGeometry();
		const client = new FakeTearoffClient();
		const h = await renderWorkspace(undefined, undefined, undefined, { tearoff: client });
		await openTwo(h);
		fireEvent.pointerDown(slot(1), { button: 0, clientX: 150, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(1), { clientX: 500, clientY: -20, pointerId: 1 });
		await waitFor(() => expect(pills()).toHaveLength(1));
		expect(pills()[0]).toHaveTextContent('Release to open in a new window');
		expect(cards()).toHaveLength(1);
		expect(cards()[0]).toHaveTextContent('docs');
		expect(cards()[0]).toHaveAttribute('aria-hidden', 'true');
		await waitFor(() => expect(status()).toContain('Drag: release to open in a new window'));
		fireEvent.pointerUp(slot(1), { clientX: 500, clientY: -20, pointerId: 1 });
		await waitFor(() => expect(status()).toContain('Moved docs to a new window'));
		await waitFor(async () =>
			expect((await h.tabs.getSnapshot()).tabs.map((tab) => tab.id)).toEqual([1, 3]),
		);
		expect(h.tabs.store.windowLabels()).toHaveLength(2);
		expect(cards()).toHaveLength(0);
		expect(pills()).toHaveLength(0);
	});

	it('moves nothing on Escape or when the pointer returns to the strip', async () => {
		stubGeometry();
		const client = new FakeTearoffClient();
		const h = await renderWorkspace(undefined, undefined, undefined, { tearoff: client });
		await openTwo(h);
		fireEvent.pointerDown(slot(1), { button: 0, clientX: 150, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(1), { clientX: 500, clientY: -20, pointerId: 1 });
		await waitFor(() => expect(cards()).toHaveLength(1));
		fireEvent.keyDown(window, { key: 'Escape' });
		await waitFor(() => expect(cards()).toHaveLength(0));
		fireEvent.pointerUp(slot(1), { clientX: 500, clientY: -20, pointerId: 1 });
		expect((await h.tabs.getSnapshot()).tabs).toHaveLength(3);

		fireEvent.pointerDown(slot(1), { button: 0, clientX: 150, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(1), { clientX: 500, clientY: -20, pointerId: 1 });
		await waitFor(() => expect(cards()).toHaveLength(1));
		fireEvent.pointerMove(slot(1), { clientX: 165, clientY: 20, pointerId: 1 });
		await waitFor(() => expect(cards()).toHaveLength(0));
		fireEvent.pointerUp(slot(1), { clientX: 165, clientY: 20, pointerId: 1 });
		expect(h.tabs.store.windowLabels()).toHaveLength(1);
	});

	it('hands over to the ghost outside the window and opens the window where the cursor was', async () => {
		stubGeometry();
		const client = new FakeTearoffClient({ ghost: true, cursorFollow: true, windowPosition: true });
		client.report = reportAt(1500, 400, 1);
		const h = await renderWorkspace(undefined, undefined, undefined, { tearoff: client });
		await openTwo(h);
		await act(async () => {});
		fireEvent.pointerDown(slot(1), { button: 0, clientX: 150, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(1), { clientX: 1500, clientY: 400, pointerId: 1 });
		await waitFor(() => expect(client.calls).toContain('begin'));
		await waitFor(() => expect(cards()).toHaveLength(0));
		fireEvent.pointerUp(slot(1), { clientX: 1500, clientY: 400, pointerId: 1 });
		await waitFor(() => expect(status()).toContain('Moved docs to a new window'));
		expect(client.calls).toContain('end:drop');
	});

	it('tears off one pane of a pair by its grip and leaves the other behind', async () => {
		stubGeometry();
		const client = new FakeTearoffClient();
		const h = await renderWorkspace(undefined, undefined, undefined, { tearoff: client });
		await openTwo(h);
		await h.tabs.joinPair([1, 2], 'sideBySide');
		await h.tabs.activateTab(1);
		await waitFor(() => expect(document.querySelectorAll('[data-pane]')).toHaveLength(2));
		const grips = () => document.querySelectorAll<HTMLElement>('[data-pane-grip]');
		fireEvent.pointerDown(grips()[1]!, { button: 0, clientX: 600, clientY: 230, pointerId: 1 });
		fireEvent.pointerMove(grips()[1]!, { clientX: 600, clientY: -20, pointerId: 1 });
		await waitFor(() => expect(cards()).toHaveLength(1));
		expect(cards()[0]).toHaveTextContent('docs');
		expect(cards()[0]).not.toHaveTextContent('2');
		fireEvent.pointerUp(grips()[1]!, { clientX: 600, clientY: -20, pointerId: 1 });
		await waitFor(() => expect(status()).toContain('Moved docs to a new window'));
		const snapshot = await h.tabs.getSnapshot();
		expect(snapshot.tabs.map((tab) => tab.id)).toEqual([1, 3]);
		expect(snapshot.pairs).toHaveLength(0);
	});

	it('does nothing outside the strip where there is no plugin', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await openTwo(h);
		fireEvent.pointerDown(slot(1), { button: 0, clientX: 150, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(1), { clientX: 500, clientY: -20, pointerId: 1 });
		expect(cards()).toHaveLength(0);
		fireEvent.pointerUp(slot(1), { clientX: 500, clientY: -20, pointerId: 1 });
		expect((await h.tabs.getSnapshot()).tabs).toHaveLength(3);
	});
});
