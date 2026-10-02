// Verifies a double-click on the tab strip's empty space opens a tab at the end, and nothing else does
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { stubLayout } from '../test/browseHarness';
import { DOCS, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

const tabs = () => screen.getAllByRole('tab');
const scroller = () => document.querySelector<HTMLElement>('[data-strip]')!;

describe('double-clicking the tab strip', () => {
	it('opens a tab at the end, in the current folder, and activates it', async () => {
		const h = await renderWorkspace();
		await h.tabs.openTab(DOCS);
		await waitFor(() => expect(tabs()).toHaveLength(2));
		const before = await h.tabs.getSnapshot();
		const activeLocation = before.tabs.find((t) => t.id === before.active)!.location;
		fireEvent.doubleClick(scroller());
		await waitFor(() => expect(tabs()).toHaveLength(3));
		const after = await h.tabs.getSnapshot();
		const last = after.tabs[after.tabs.length - 1]!;
		expect(after.active).toBe(last.id);
		expect(last.location).toEqual(activeLocation);
	});

	it('opens the tab at the end even when the active tab is not the last', async () => {
		const h = await renderWorkspace();
		await h.tabs.openTab(DOCS);
		await waitFor(() => expect(tabs()).toHaveLength(2));
		await h.tabs.activateTab((await h.tabs.getSnapshot()).tabs[0]!.id);
		const firstId = (await h.tabs.getSnapshot()).tabs[0]!.id;
		fireEvent.doubleClick(scroller());
		await waitFor(() => expect(tabs()).toHaveLength(3));
		const ids = (await h.tabs.getSnapshot()).tabs.map((t) => t.id);
		expect(ids[0]).toBe(firstId);
		expect((await h.tabs.getSnapshot()).active).toBe(ids[ids.length - 1]);
	});

	it('does nothing on a tab, the + button or a close button', async () => {
		const h = await renderWorkspace();
		await h.tabs.openTab(DOCS);
		await waitFor(() => expect(tabs()).toHaveLength(2));
		fireEvent.doubleClick(tabs()[0]!);
		fireEvent.doubleClick(screen.getByRole('button', { name: 'New tab' }));
		fireEvent.doubleClick(screen.getAllByRole('button', { name: /^Close / })[0]!);
		await new Promise((resolve) => setTimeout(resolve, 30));
		expect(tabs()).toHaveLength(2);
	});
});
