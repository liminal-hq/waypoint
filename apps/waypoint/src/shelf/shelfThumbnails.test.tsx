// Verifies thumbnails on the Shelf and in the drag stack: asked for files only, shown as decoration, carried into a drag
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { dismissNotice } from '../app/notices';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { FakeTabsStore } from '../services/fakeTabsStore';
import { fileLocation } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME, renderWorkspace } from '../test/workspaceHarness';
import { createFakeThumbnailsClient } from '../thumbnails/fakeThumbnailsClient';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
	localStorage.clear();
	document.elementsFromPoint = () => [];
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
	// @ts-expect-error happy-dom has no hit test; the tests stand one in
	delete document.elementsFromPoint;
});

async function mount() {
	const thumbs = createFakeThumbnailsClient();
	const tabs = new FakeTabsApi(FakeTabsStore.singleWindow(), 'main-1');
	await tabs.openTab(HOME);
	await tabs.addToShelf([fileLocation('/home/test/photo.jpg'), fileLocation('/home/test/docs')]);
	await renderWorkspace(createTree(), tabs, undefined, {
		thumbnails: thumbs,
		ops: createFakeOpsClient(),
	});
	fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
	const panel = await screen.findByRole('complementary', { name: 'Shelf' });
	return { thumbs, panel };
}

const askedFor = (thumbs: ReturnType<typeof createFakeThumbnailsClient>) =>
	thumbs.batches.filter((batch) => batch.handle === null).flatMap((batch) => batch.keys);

describe('the Shelf', () => {
	it('asks for pictures of its files, never its folders, and draws them as decoration', async () => {
		const { thumbs, panel } = await mount();
		await waitFor(() => expect(askedFor(thumbs)).toContain('file:///home/test/photo.jpg'));
		expect(askedFor(thumbs).some((key) => key.endsWith('/docs'))).toBe(false);
		act(() => thumbs.ready('file:///home/test/photo.jpg', 'thumb://localhost/normal/p.png'));
		await waitFor(() => expect(panel.querySelectorAll('img')).toHaveLength(1));
		const image = panel.querySelector('img')!;
		expect(image.getAttribute('alt')).toBe('');
		// The name is still the item's.
		expect(image.closest('[role="treeitem"]')).toHaveTextContent('photo.jpg');
	});

	it('keeps the icon where there is no thumbnail', async () => {
		const { thumbs, panel } = await mount();
		await waitFor(() => expect(askedFor(thumbs).length).toBeGreaterThan(0));
		act(() => thumbs.skip('file:///home/test/photo.jpg', 'noGenerator'));
		expect(panel.querySelectorAll('img')).toHaveLength(0);
		expect(panel.querySelectorAll('[role="treeitem"][aria-level="2"]')).toHaveLength(2);
		expect(panel.querySelectorAll('svg[data-group]').length).toBeGreaterThanOrEqual(2);
	});

	it('puts the thumbnail of a dragged item in the stack', async () => {
		const { thumbs, panel } = await mount();
		await waitFor(() => expect(askedFor(thumbs)).toContain('file:///home/test/photo.jpg'));
		act(() => thumbs.ready('file:///home/test/photo.jpg', 'thumb://localhost/normal/p.png'));
		await waitFor(() => expect(panel.querySelectorAll('img')).toHaveLength(1));
		const item = panel.querySelector('[data-shelf-item]')!;
		const photo = [...panel.querySelectorAll('[role="treeitem"]')].find((row) =>
			row.textContent?.includes('photo.jpg'),
		)!;
		expect(item).not.toBeNull();
		fireEvent.pointerDown(photo, { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
		fireEvent.pointerMove(window, { pointerId: 1, clientX: 140, clientY: 100 });
		await waitFor(() => expect(document.querySelector('[data-drag-stack]')).not.toBeNull());
		const card = document.querySelector<HTMLImageElement>('[data-drag-stack] img');
		expect(card?.getAttribute('src')).toBe('thumb://localhost/normal/p.png');
		expect(card?.getAttribute('alt')).toBe('');
		fireEvent.pointerUp(window, { pointerId: 1, clientX: 140, clientY: 100 });
	});
});
