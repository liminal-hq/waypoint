// Verifies the drag over the real strip: the pill, the ring, the edge zone, Esc, and the pane grip
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { stubLayout } from '../test/browseHarness';
import { endRename } from './groupActions';
import { DOCS, MUSIC, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	// The name field a new group opens is module state; one test's must not reach the next.
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

/** Tabs 100 wide, a strip 0..30 tall, a file area 200..600 tall, and a chip left of the tabs. */
function stubGeometry() {
	return vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
		this: HTMLElement,
	) {
		const rect = (left: number, top: number, right: number, bottom: number) =>
			({ left, top, right, bottom, width: right - left, height: bottom - top }) as DOMRect;
		const index = this.getAttribute('data-index');
		if (index !== null) return rect(Number(index) * 100, 0, Number(index) * 100 + 100, 30);
		if (this.hasAttribute('data-chip')) return rect(-100, 0, -10, 30);
		if (this.hasAttribute('data-strip')) return rect(0, 0, 2000, 30);
		if (this.hasAttribute('data-pane-area')) return rect(0, 200, 1000, 600);
		return rect(0, 0, 100, 30);
	});
}

async function openTwo(h: Harness) {
	await h.tabs.openTab(DOCS);
	await h.tabs.openTab(MUSIC);
	await waitFor(() => expect(tabs()).toHaveLength(3));
}

const status = () =>
	screen
		.getAllByRole('status')
		.map((node) => node.textContent)
		.join('|');

describe('a tab drag', () => {
	it('shows exactly one pill, which says Esc to cancel, and announces it through the live region', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await openTwo(h);
		fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, clientY: 15, pointerId: 1 });
		expect(pills()).toHaveLength(0);
		fireEvent.pointerMove(slot(0), { clientX: 262, clientY: 15, pointerId: 1 });
		await waitFor(() => expect(pills()).toHaveLength(1));
		expect(pills()[0]).toHaveTextContent('Release to move test to position 3 of 3');
		expect(pills()[0]).toHaveTextContent('Esc to cancel');
		// The pill is drawn, not announced twice: it is hidden from assistive technology.
		expect(pills()[0]).toHaveAttribute('aria-hidden', 'true');
		await waitFor(() =>
			expect(status()).toContain('Drag: release to move test to position 3 of 3'),
		);
		expect(screen.getByRole('tablist')).toHaveAttribute('data-drag', 'active');
		fireEvent.pointerUp(slot(0), { clientX: 262, clientY: 15, pointerId: 1 });
		await waitFor(() => expect(pills()).toHaveLength(0));
		await waitFor(() => expect(status()).toContain('Moved test to position 3 of 3'));
	});

	it('draws a ring over the tab held over, bridges after 450 ms, and splits on release', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await openTwo(h);
		fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(0), { clientX: 150, clientY: 15, pointerId: 1 });
		await waitFor(() => expect(slot(1)).toHaveAttribute('data-ring', 'drawing'));
		expect(pills()[0]).toHaveTextContent('Release to move test');
		await waitFor(() => expect(slot(1)).toHaveAttribute('data-ring', 'armed'), { timeout: 1500 });
		expect(slot(0)).toHaveAttribute('data-bridge', 'start');
		expect(slot(1)).toHaveAttribute('data-bridge', 'end');
		expect(pills()).toHaveLength(1);
		expect(pills()[0]).toHaveTextContent('Release to split with docs');
		fireEvent.pointerUp(slot(0), { clientX: 150, clientY: 15, pointerId: 1 });
		await waitFor(async () => expect((await h.tabs.getSnapshot()).pairs).toHaveLength(1));
		expect((await h.tabs.getSnapshot()).pairs[0]!.panes).toEqual([1, 2]);
		await waitFor(() => expect(status()).toContain('Split test and docs'));
	});

	it('draws a bracket after 800 ms of rest and creates the group, ready to rename', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await openTwo(h);
		// Drag music, the last tab, a little way; its slot has a neighbour to group with.
		fireEvent.pointerDown(slot(2), { button: 0, clientX: 250, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(2), { clientX: 262, clientY: 15, pointerId: 1 });
		await waitFor(() => expect(pills()[0]).toHaveTextContent('Release to start a new group'), {
			timeout: 2000,
		});
		expect(document.querySelector('[data-kind="bracket"]')).not.toBeNull();
		fireEvent.pointerUp(slot(2), { clientX: 262, clientY: 15, pointerId: 1 });
		await waitFor(async () => expect((await h.tabs.getSnapshot()).groups).toHaveLength(1));
		const group = (await h.tabs.getSnapshot()).groups[0]!;
		// The new group's name opens for editing.
		const field = await screen.findByRole('textbox', { name: 'Group name' });
		expect(field).toHaveValue(group.name);
	});

	it('highlights an edge zone with the pane area and splits on release', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await openTwo(h);
		fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(0), { clientX: 40, clientY: 400, pointerId: 1 });
		await waitFor(() => expect(document.querySelector('[data-edge="left"]')).not.toBeNull());
		expect(pills()[0]).toHaveTextContent('Release to split on the left');
		fireEvent.pointerUp(slot(0), { clientX: 40, clientY: 400, pointerId: 1 });
		await waitFor(async () => expect((await h.tabs.getSnapshot()).pairs).toHaveLength(1));
		const pair = (await h.tabs.getSnapshot()).pairs[0]!;
		expect(pair.layout).toBe('sideBySide');
		expect(pair.panes).toEqual([1, 3]);
		await waitFor(() => expect(document.querySelector('[data-edge]')).toBeNull());
	});

	it('shows no pill in the new-window phase and does not cancel', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await openTwo(h);
		fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(0), { clientX: 500, clientY: 100, pointerId: 1 });
		await waitFor(() => expect(slot(0)).toHaveAttribute('data-dragging'));
		expect(pills()).toHaveLength(0);
		fireEvent.pointerUp(slot(0), { clientX: 500, clientY: 100, pointerId: 1 });
		expect((await h.tabs.getSnapshot()).tabs.map((tab) => tab.id)).toEqual([1, 2, 3]);
	});

	it('is abandoned by Esc: the tab slides back, nothing changes and it says so', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await openTwo(h);
		fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(0), { clientX: 262, clientY: 15, pointerId: 1 });
		await waitFor(() => expect(pills()).toHaveLength(1));
		fireEvent.keyDown(window, { key: 'Escape' });
		await waitFor(() => expect(pills()).toHaveLength(0));
		// While it slides back, the whole strip transitions; the dragged slot is at rest again.
		expect(screen.getByRole('tablist')).toHaveAttribute('data-drag', 'settling');
		expect(slot(0).style.getPropertyValue('--wp-tab-shift')).toBe('0px');
		await waitFor(() => expect(status()).toContain('Drag cancelled'));
		fireEvent.pointerUp(slot(0), { clientX: 262, clientY: 15, pointerId: 1 });
		expect((await h.tabs.getSnapshot()).tabs.map((tab) => tab.id)).toEqual([1, 2, 3]);
		await waitFor(() => expect(screen.getByRole('tablist')).not.toHaveAttribute('data-drag'));
		expect(slot(0)).not.toHaveAttribute('data-dragging');
	});

	it('does not toggle a group when its chip is dragged, but still toggles on a click', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await openTwo(h);
		const id = await h.tabs.createGroup([1, 2]);
		const chip = await screen.findByRole('button', { name: /Group 1, tab group/ });
		fireEvent.click(chip);
		await waitFor(async () =>
			expect((await h.tabs.getSnapshot()).groups.find((g) => g.id === id)?.collapsed).toBe(true),
		);
		fireEvent.click(chip);
		await waitFor(async () =>
			expect((await h.tabs.getSnapshot()).groups.find((g) => g.id === id)?.collapsed).toBe(false),
		);
		const wrap = chip.parentElement!;
		fireEvent.pointerDown(wrap, { button: 0, clientX: -50, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(wrap, { clientX: 330, clientY: 15, pointerId: 1 });
		await waitFor(() => expect(pills()[0]).toHaveTextContent('Release to move group Group 1'));
		fireEvent.pointerUp(wrap, { clientX: 330, clientY: 15, pointerId: 1 });
		fireEvent.click(chip);
		await waitFor(async () =>
			expect((await h.tabs.getSnapshot()).tabs.map((tab) => tab.id)).toEqual([3, 1, 2]),
		);
		expect((await h.tabs.getSnapshot()).groups.find((g) => g.id === id)?.collapsed).toBe(false);
	});
});

describe('a pair', () => {
	async function pair(h: Harness) {
		await openTwo(h);
		await h.tabs.joinPair([1, 2], 'sideBySide');
		await h.tabs.activateTab(1);
		await waitFor(() => expect(document.querySelectorAll('[data-pane]')).toHaveLength(2));
	}

	it('moves both tabs when its joint is dragged', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await pair(h);
		const joint = screen.getByRole('button', { name: /^Split: test and docs$/ });
		fireEvent.pointerDown(joint, { button: 0, clientX: 190, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(joint, { clientX: 460, clientY: 15, pointerId: 1 });
		await waitFor(() =>
			expect(pills()[0]).toHaveTextContent('Release to move Split: test and docs'),
		);
		fireEvent.pointerUp(joint, { clientX: 460, clientY: 15, pointerId: 1 });
		await waitFor(async () =>
			expect((await h.tabs.getSnapshot()).tabs.map((tab) => tab.id)).toEqual([3, 1, 2]),
		);
	});

	it('is separated by dragging a pane’s grip up to the strip, and not by dragging it elsewhere', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await pair(h);
		const grips = () => document.querySelectorAll<HTMLElement>('[data-pane-grip]');
		expect(grips()).toHaveLength(2);
		fireEvent.pointerDown(grips()[1]!, { button: 0, clientX: 600, clientY: 230, pointerId: 1 });
		fireEvent.pointerMove(grips()[1]!, { clientX: 600, clientY: 300, pointerId: 1 });
		expect(pills()).toHaveLength(0);
		fireEvent.pointerUp(grips()[1]!, { clientX: 600, clientY: 300, pointerId: 1 });
		expect((await h.tabs.getSnapshot()).pairs).toHaveLength(1);

		fireEvent.pointerDown(grips()[1]!, { button: 0, clientX: 600, clientY: 230, pointerId: 1 });
		fireEvent.pointerMove(grips()[1]!, { clientX: 600, clientY: 20, pointerId: 1 });
		await waitFor(() => expect(pills()[0]).toHaveTextContent('Release to separate the split'));
		await waitFor(() => expect(status()).toContain('Drag: release to separate the split'));
		fireEvent.pointerUp(grips()[1]!, { clientX: 600, clientY: 20, pointerId: 1 });
		await waitFor(async () => expect((await h.tabs.getSnapshot()).pairs).toHaveLength(0));
		await waitFor(() => expect(status()).toContain('Separated test and docs'));
	});

	it('leaves the pair alone when the grip drag is cancelled', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await pair(h);
		const grip = document.querySelector<HTMLElement>('[data-pane-grip]')!;
		fireEvent.pointerDown(grip, { button: 0, clientX: 600, clientY: 230, pointerId: 1 });
		fireEvent.pointerMove(grip, { clientX: 600, clientY: 20, pointerId: 1 });
		await waitFor(() => expect(pills()).toHaveLength(1));
		fireEvent.keyDown(window, { key: 'Escape' });
		fireEvent.pointerUp(grip, { clientX: 600, clientY: 20, pointerId: 1 });
		expect((await h.tabs.getSnapshot()).pairs).toHaveLength(1);
		await waitFor(() => expect(pills()).toHaveLength(0));
	});
});

describe('the strip while a drag ends in someone else’s hands', () => {
	it('cancels a drag when the session changes under it', async () => {
		stubGeometry();
		const h = await renderWorkspace();
		await openTwo(h);
		fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, clientY: 15, pointerId: 1 });
		fireEvent.pointerMove(slot(0), { clientX: 262, clientY: 15, pointerId: 1 });
		await waitFor(() => expect(pills()).toHaveLength(1));
		await act(async () => {
			await h.tabs.closeTab(2);
		});
		await waitFor(() => expect(pills()).toHaveLength(0));
		fireEvent.pointerUp(slot(0), { clientX: 262, clientY: 15, pointerId: 1 });
		expect((await h.tabs.getSnapshot()).tabs.map((tab) => tab.id)).toEqual([1, 3]);
		expect(within(screen.getByRole('tablist')).getAllByRole('tab')).toHaveLength(2);
	});
});
