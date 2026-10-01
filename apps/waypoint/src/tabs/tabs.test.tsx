// Verifies the tab strip over the session: roles, open, close, switch, reorder, keys and overflow
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { stubLayout } from '../test/browseHarness';
import { DOCS, HOME, MUSIC, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

type Harness = Awaited<ReturnType<typeof renderWorkspace>>;

const tabs = () => screen.getAllByRole('tab');
const titles = () => tabs().map((tab) => tab.textContent);
const option = (name: string) => screen.findByRole('option', { name: new RegExp(`^${name}`) });
const snapshot = (h: Harness) => h.tabs.getSnapshot();

async function openTwo(h: Harness) {
	await h.tabs.openTab(DOCS);
	await h.tabs.openTab(MUSIC);
	await waitFor(() => expect(tabs()).toHaveLength(3));
}

describe('the tab strip', () => {
	it('is a tablist whose tabs are named for their folder and one is selected', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		expect(screen.getByRole('tablist', { name: 'Tabs' })).toBeInTheDocument();
		await waitFor(() => expect(titles()).toEqual(['test', 'docs', 'music']));
		expect(tabs().map((tab) => tab.getAttribute('aria-selected'))).toEqual([
			'false',
			'false',
			'true',
		]);
		expect(tabs()[2]).toHaveAttribute('title', '/home/test/music');
		const panel = screen.getByRole('tabpanel');
		expect(panel).toHaveAttribute('aria-labelledby', tabs()[2]!.id);
		expect(tabs()[2]).toHaveAttribute('aria-controls', panel.id);
	});

	it('uses roving focus: only the focus stop is in the tab order', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		expect(tabs().map((tab) => tab.tabIndex)).toEqual([-1, -1, 0]);
	});

	it('switches the file view on click, and keeps each tab’s selection', async () => {
		const h = await renderWorkspace();
		await option('notes.txt');
		fireEvent.click(await option('notes.txt'));
		await h.tabs.openTab(DOCS);
		await option('report.pdf');
		expect(screen.queryByRole('option', { name: /^notes/ })).toBeNull();

		fireEvent.click(tabs()[0]!);
		const notes = await option('notes.txt');
		expect(notes).toHaveAttribute('aria-selected', 'true');
		expect(tabs()[0]).toHaveAttribute('aria-selected', 'true');
	});

	it('opens a tab at the current folder with +, and at Home with middle-click', async () => {
		const h = await renderWorkspace();
		await h.tabs.navigate(1, DOCS);
		await option('report.pdf');
		fireEvent.click(screen.getByRole('button', { name: 'New tab' }));
		await waitFor(() => expect(tabs()).toHaveLength(2));
		expect((await snapshot(h)).tabs[1]!.location.uri).toBe(DOCS.uri);
		expect(tabs()[1]).toHaveAttribute('aria-selected', 'true');

		fireEvent(
			screen.getByRole('button', { name: 'New tab' }),
			new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true }),
		);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		expect((await snapshot(h)).tabs[2]!.location.uri).toBe(HOME.uri);
	});

	it('closes with the button or a middle-click, and releases the closed tab’s listing', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await option('song.mp3');
		// Each tab that has been active still holds its listing; only its pages are evicted later.
		expect(h.client.openCount).toBe(3);

		fireEvent.click(screen.getByRole('button', { name: 'Close music' }));
		await waitFor(() => expect(tabs()).toHaveLength(2));
		expect(h.client.openCount).toBe(2);
		// Closing the active tab activates its neighbour.
		expect(tabs()[1]).toHaveAttribute('aria-selected', 'true');
		await option('report.pdf');

		fireEvent(
			tabs()[0]!.parentElement!,
			new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true }),
		);
		await waitFor(() => expect(tabs()).toHaveLength(1));
		await waitFor(() => expect(h.client.openCount).toBe(1));
	});

	it('closes the window when the last tab closes, and the tab goes to Recently Closed', async () => {
		const onLastWindowClosed = vi.fn();
		const store = new FakeTabsStore({
			policy: { closeWindowOnLastTab: true },
			onLastWindowClosed,
		});
		const h = await renderWorkspace(undefined, new FakeTabsApi(store, 'main-1'));
		await h.tabs.navigate(1, DOCS);
		await option('report.pdf');
		fireEvent.click(screen.getByRole('button', { name: 'Close docs' }));
		await waitFor(() => expect(onLastWindowClosed).toHaveBeenCalledTimes(1));
		expect(store.windowLabels()).toEqual([]);
		expect(store.closed().map((closed) => closed.tab.location.uri)).toEqual([DOCS.uri]);
	});

	it('opens a folder in a background tab on middle-click, and leaves files alone', async () => {
		const h = await renderWorkspace();
		const notes = await option('notes.txt');
		const aux = () => new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true });
		fireEvent(notes, aux());
		expect(await snapshot(h)).toMatchObject({ tabs: [{}] });

		fireEvent(await option('docs'), aux());
		await waitFor(() => expect(tabs()).toHaveLength(2));
		const s = await snapshot(h);
		expect(s.tabs[1]!.location.uri).toBe(DOCS.uri);
		expect(s.active).toBe(1);
		expect(tabs()[0]).toHaveAttribute('aria-selected', 'true');
	});
});

describe('keyboard', () => {
	it('moves focus with the arrow keys without activating, Home and End included', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		act(() => tabs()[2]!.focus());
		fireEvent.keyDown(tabs()[2]!, { key: 'ArrowLeft' });
		expect(tabs()[1]).toHaveFocus();
		expect(tabs()[2]).toHaveAttribute('aria-selected', 'true');
		fireEvent.keyDown(tabs()[1]!, { key: 'Home' });
		expect(tabs()[0]).toHaveFocus();
		fireEvent.keyDown(tabs()[0]!, { key: 'ArrowLeft' });
		expect(tabs()[0]).toHaveFocus();
		fireEvent.keyDown(tabs()[0]!, { key: 'End' });
		expect(tabs()[2]).toHaveFocus();
		fireEvent.keyDown(tabs()[2]!, { key: 'ArrowLeft' });
		fireEvent.keyDown(tabs()[1]!, { key: 'Enter' });
		await waitFor(() => expect(tabs()[1]).toHaveAttribute('aria-selected', 'true'));
		expect(tabs().map((tab) => tab.tabIndex)).toEqual([-1, 0, -1]);
	});

	it('reorders with Ctrl+Shift+arrows and announces it', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		act(() => tabs()[0]!.focus());
		fireEvent.keyDown(tabs()[0]!, { key: 'ArrowRight', ctrlKey: true, shiftKey: true });
		await waitFor(() => expect(titles()).toEqual(['docs', 'test', 'music']));
		expect(screen.getByText(/Moved \/home\/test to position 2 of 3/)).toBeInTheDocument();
		// At the end it does nothing.
		act(() => tabs()[2]!.focus());
		fireEvent.keyDown(tabs()[2]!, { key: 'ArrowRight', ctrlKey: true, shiftKey: true });
		expect(titles()).toEqual(['docs', 'test', 'music']);
	});

	it('closes the focused tab with Delete', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		fireEvent.keyDown(tabs()[1]!, { key: 'Delete' });
		await waitFor(() => expect(tabs()).toHaveLength(2));
	});

	it('handles Ctrl+T, Ctrl+W, Ctrl+Tab, Ctrl+Shift+Tab and Alt+digits', async () => {
		const h = await renderWorkspace();
		await h.tabs.navigate(1, DOCS);
		await option('report.pdf');

		fireEvent.keyDown(window, { key: 't', ctrlKey: true });
		await waitFor(() => expect(tabs()).toHaveLength(2));
		expect((await snapshot(h)).tabs[1]!.location.uri).toBe(DOCS.uri);

		fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true });
		await waitFor(() => expect(tabs()[0]).toHaveAttribute('aria-selected', 'true'));
		fireEvent.keyDown(window, { key: 'Tab', ctrlKey: true, shiftKey: true });
		await waitFor(() => expect(tabs()[1]).toHaveAttribute('aria-selected', 'true'));

		fireEvent.keyDown(window, { key: '1', altKey: true });
		await waitFor(() => expect(tabs()[0]).toHaveAttribute('aria-selected', 'true'));
		fireEvent.keyDown(window, { key: '9', altKey: true });
		expect(tabs()[0]).toHaveAttribute('aria-selected', 'true');
		fireEvent.keyDown(window, { key: '2', altKey: true });
		await waitFor(() => expect(tabs()[1]).toHaveAttribute('aria-selected', 'true'));

		fireEvent.keyDown(window, { key: 'w', ctrlKey: true });
		await waitFor(() => expect(tabs()).toHaveLength(1));
	});
});

describe('the focus stop', () => {
	it('returns to the active tab when another route changes it', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		act(() => tabs()[2]!.focus());
		fireEvent.keyDown(tabs()[2]!, { key: 'Home' });
		expect(tabs().map((tab) => tab.tabIndex)).toEqual([0, -1, -1]);
		fireEvent.keyDown(window, { key: '2', altKey: true });
		await waitFor(() => expect(tabs()[1]).toHaveAttribute('aria-selected', 'true'));
		expect(tabs().map((tab) => tab.tabIndex)).toEqual([-1, 0, -1]);
	});

	it('closes once on a middle-click of the close button', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		const close = vi.spyOn(h.tabs, 'closeTab');
		fireEvent(
			screen.getByRole('button', { name: 'Close docs' }),
			new MouseEvent('auxclick', { button: 1, bubbles: true, cancelable: true }),
		);
		await waitFor(() => expect(tabs()).toHaveLength(2));
		expect(close).toHaveBeenCalledTimes(1);
	});
});

describe('dragging to reorder', () => {
	function stubSpans() {
		return vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
			this: HTMLElement,
		) {
			const index = this.getAttribute('data-index');
			const left = index === null ? 0 : Number(index) * 100;
			return { left, right: left + 100, top: 0, bottom: 30, width: 100, height: 30 } as DOMRect;
		});
	}

	const slot = (index: number) => tabs()[index]!.parentElement!;

	it('moves a tab to where it is dropped', async () => {
		stubSpans();
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, pointerId: 1 });
		fireEvent.pointerMove(slot(0), { clientX: 60, pointerId: 1 });
		fireEvent.pointerMove(slot(0), { clientX: 260, pointerId: 1 });
		expect(slot(0)).toHaveAttribute('data-dragging');
		fireEvent.pointerUp(slot(0), { clientX: 260, pointerId: 1 });
		await waitFor(() => expect(titles()).toEqual(['docs', 'music', 'test']));
		// The click that follows a drag does not activate anything.
		fireEvent.click(slot(2));
		expect(tabs()[0]).toHaveAttribute('aria-selected', 'false');
	});

	it('commits a drop whose pointerup arrives before React has rendered the last move', async () => {
		stubSpans();
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		act(() => {
			fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, pointerId: 1 });
			fireEvent.pointerMove(slot(0), { clientX: 260, pointerId: 1 });
			fireEvent.pointerUp(slot(0), { clientX: 260, pointerId: 1 });
		});
		await waitFor(() => expect(titles()).toEqual(['docs', 'music', 'test']));
		fireEvent.click(slot(2));
		expect(tabs()[0]).toHaveAttribute('aria-selected', 'false');
	});

	it('treats a small movement as a click, and Escape abandons a drag', async () => {
		stubSpans();
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, pointerId: 1 });
		fireEvent.pointerMove(slot(0), { clientX: 52, pointerId: 1 });
		fireEvent.pointerUp(slot(0), { clientX: 52, pointerId: 1 });
		fireEvent.click(slot(0));
		await waitFor(() => expect(tabs()[0]).toHaveAttribute('aria-selected', 'true'));

		fireEvent.pointerDown(slot(0), { button: 0, clientX: 50, pointerId: 1 });
		fireEvent.pointerMove(slot(0), { clientX: 280, pointerId: 1 });
		fireEvent.keyDown(window, { key: 'Escape' });
		fireEvent.pointerUp(slot(0), { clientX: 280, pointerId: 1 });
		expect(titles()).toEqual(['test', 'docs', 'music']);
	});
});

describe('overflow', () => {
	it('shows scroll arrows only when the tabs overflow and only toward hidden tabs', async () => {
		const h = await renderWorkspace();
		await openTwo(h);
		await waitFor(() => expect(tabs()).toHaveLength(3));
		const scroller = screen.getByRole('tablist').parentElement!;
		const scrollLeft = screen.getByLabelText('Scroll tabs left');
		const scrollRight = screen.getByLabelText('Scroll tabs right');
		expect(scrollLeft).not.toBeVisible();
		expect(scrollRight).not.toBeVisible();

		Object.defineProperty(scroller, 'scrollWidth', { configurable: true, value: 900 });
		Object.defineProperty(scroller, 'clientWidth', { configurable: true, value: 400 });
		fireEvent.scroll(scroller);
		await waitFor(() => expect(scrollRight).toBeVisible());
		expect(scrollLeft).not.toBeVisible();

		fireEvent.click(scrollRight);
		scroller.scrollLeft = 500;
		fireEvent.scroll(scroller);
		await waitFor(() => expect(scrollLeft).toBeVisible());
		expect(scrollRight).not.toBeVisible();
	});
});
