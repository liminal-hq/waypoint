// Verifies the landing line on the real strip: another window's drag over it, the line's place, and the drop landing there
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTearoffClient } from '../services/fakeTearoffClient';
import { stubLayout } from '../test/browseHarness';
import { DOCS, MUSIC, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
	// Tabs 100 wide in a strip 0..800 tall 30; the tablist starts at 0.
	vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
		this: HTMLElement,
	) {
		const rect = (left: number, top: number, right: number, bottom: number) =>
			({ left, top, right, bottom, width: right - left, height: bottom - top }) as DOMRect;
		const index = this.getAttribute('data-index');
		if (index !== null) return rect(Number(index) * 100, 0, Number(index) * 100 + 100, 30);
		if (this.hasAttribute('data-strip') || this.getAttribute('role') === 'tablist') {
			return rect(0, 0, 800, 30);
		}
		return rect(0, 0, 100, 30);
	});
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

const landingLine = () => document.querySelector('[data-landing][aria-hidden="true"]');
const status = () =>
	screen
		.getAllByRole('status')
		.map((node) => node.textContent)
		.join('|');

async function open() {
	const client = new FakeTearoffClient({ hitTest: true });
	const h = await renderWorkspace(undefined, undefined, undefined, { tearoff: client });
	await h.tabs.openTab(DOCS);
	await h.tabs.openTab(MUSIC);
	await waitFor(() => expect(screen.getAllByRole('tab')).toHaveLength(3));
	return { client, h };
}

describe('the strip while another window drags tabs over it', () => {
	it('draws an aria-hidden line at the slot and highlights the strip, then clears them', async () => {
		const { client } = await open();
		expect(landingLine()).toBeNull();
		act(() => client.fireMergeHover({ x: 150, y: 10, region: 'slot:2', count: 1, pinned: false }));
		const line = landingLine() as HTMLElement;
		expect(line).not.toBeNull();
		expect(line).toHaveAttribute('aria-hidden', 'true');
		expect(line.style.left).toBe('200px');
		expect(document.querySelector('[data-strip]')).toHaveAttribute('data-landing');
		act(() => client.fireMergeLeave());
		expect(landingLine()).toBeNull();
		expect(document.querySelector('[data-strip]')).not.toHaveAttribute('data-landing');
	});

	it('moves the line with the slot and says where once in the live region', async () => {
		const { client } = await open();
		act(() => client.fireMergeHover({ x: 0, y: 0, region: 'slot:0', count: 1, pinned: false }));
		expect((landingLine() as HTMLElement).style.left).toBe('0px');
		act(() => client.fireMergeHover({ x: 0, y: 0, region: 'slot:3', count: 1, pinned: false }));
		expect((landingLine() as HTMLElement).style.left).toBe('300px');
		await waitFor(() =>
			expect(status()).toContain('A tab is being dragged here: release to add it at position 1'),
		);
		expect(status()).not.toContain('position 4');
	});

	it('also shows for a window drag the plugin reports, with a tear payload', async () => {
		const { client } = await open();
		act(() =>
			client.fireDragHover({
				window: 'main-1',
				x: 220,
				y: 12,
				region: 'slot:2',
				payload: {
					v: 1,
					mode: 'tabs',
					what: { kind: 'tabs', value: [9] },
					tabs: [9],
					name: 'x',
					source: { window: 'main-2', index: 0 },
				},
			}),
		);
		expect((landingLine() as HTMLElement).style.left).toBe('200px');
	});
});
