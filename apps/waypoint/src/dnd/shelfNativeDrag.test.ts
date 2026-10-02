// Verifies the Shelf and the native drag and drop meet: its rows leave the window as a system drag, and files from other applications drop onto it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { fileLocation } from '../services/fakeVfsClient';
import { disposeHarnesses, mark, nativeHarness, OUTSIDE, settle } from '../test/nativeDragHarness';
import { FILE_DRAG_ATTRIBUTE } from './fileDrag';

afterEach(() => {
	disposeHarnesses();
	document.body.innerHTML = '';
	document.documentElement.removeAttribute('style');
	document.documentElement.removeAttribute(FILE_DRAG_ATTRIBUTE);
});

const noKeys = { ctrl: false, shift: false, alt: false };

describe('a Shelf row dragged out of the window', () => {
	it('hands its own locations to the system drag, without resolving a selection', async () => {
		const h = await nativeHarness();
		let resolved = 0;
		h.state.resolve = async () => {
			resolved += 1;
			return [];
		};
		const items = [fileLocation('/home/test/a.txt'), fileLocation('/srv/other with space.txt')];
		h.startShelfDrag(items);
		h.move(-5);
		await settle();
		expect(h.started).toEqual([
			{
				uris: ['file:///home/test/a.txt', 'file:///srv/other%20with%20space.txt'],
				actions: ['copy', 'move', 'link'],
			},
		]);
		expect(resolved).toBe(0);
		expect(h.phase()).not.toBe('dragging');
	});

	it('offers no link when an item is not local', async () => {
		const h = await nativeHarness();
		h.startShelfDrag([{ display: 'host/a', uri: 'sftp://host/a' }]);
		h.move(-5);
		await settle();
		// Only `file:` URIs can be handed over, so a remote reference stays in the page with a notice.
		expect(h.started).toEqual([]);
		expect(h.say.length).toBe(1);
	});
});

describe('files from another application dropped on the Shelf', () => {
	it('adds references to the Shelf and runs no job', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		h.over(mark('shelf', 'shelf', 'Shelf'));
		feed.move({ x: 140, y: 100 }, noKeys);
		await settle();
		expect(h.target()).toMatchObject({ kind: 'shelf', outcome: 'shelf', blocked: null });
		expect(h.pill()).toBe('Add 2 items to the Shelf');
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.shelved).toEqual([OUTSIDE]);
		expect(h.transfers).toEqual([]);
		expect(h.moved).toEqual([]);
	});

	it('still refuses the Shelf’s own rows dropped back on the Shelf', async () => {
		const h = await nativeHarness();
		h.startShelfDrag();
		h.over(mark('shelf', 'shelf', 'Shelf'));
		h.move(140);
		await settle();
		expect(h.target()).toMatchObject({ blocked: { kind: 'onShelf' }, outcome: null });
		h.up(140);
		await settle();
		expect(h.shelved).toEqual([]);
	});
});
