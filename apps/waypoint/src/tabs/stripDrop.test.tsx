// Verifies the tab strip's empty space is a drop target of the same kind as the + button, and the tabs and arrows are not
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dropSpotAt } from '../dnd/dropTargets';
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

describe('dropping files on the tab strip', () => {
	it('treats the strip behind the tabs as a + drop, and leaves tabs and arrows alone', async () => {
		const h = await renderWorkspace();
		await h.tabs.openTab(DOCS);
		await waitFor(() => expect(screen.getAllByRole('tab')).toHaveLength(2));
		const strip = document.querySelector<HTMLElement>('[data-strip]')!;
		const plus = screen.getByRole('button', { name: 'New tab' });
		const point = { x: 5, y: 5 };

		// Empty space: the scroller is the innermost marked element under the pointer.
		const empty = dropSpotAt(point, { elementsFromPoint: () => [strip] });
		expect(empty).toMatchObject({ kind: 'plus', ref: 'new', label: 'New tab', inStrip: true });
		expect(empty?.element).toBe(strip);

		// The + button keeps its own, identical, target.
		const onPlus = dropSpotAt(point, { elementsFromPoint: () => [plus, plus.parentElement!] });
		expect(onPlus).toMatchObject({ kind: 'plus', ref: 'new', label: 'New tab' });
		expect(onPlus?.element).toBe(plus);

		// A tab wins over the strip behind it.
		const slot = document.querySelector<HTMLElement>('[data-slot]')!;
		expect(dropSpotAt(point, { elementsFromPoint: () => [slot, strip] })?.kind).toBe('tab');

		// The scroll arrows sit outside the scroller and are not targets.
		const arrows = document.querySelectorAll<HTMLElement>('button[aria-label^="Scroll tabs"]');
		expect(arrows).toHaveLength(2);
		for (const arrow of arrows) {
			expect(
				dropSpotAt(point, { elementsFromPoint: () => [arrow, arrow.parentElement!] }),
			).toBeNull();
		}
	});
});
