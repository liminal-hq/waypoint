// Verifies what a drop asks of the file commands: transferring a selection into a folder, and Link To…
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { FOLDER } from '../test/browseHarness';
import { clipboardHarness, OTHER, selectIn, type ClipboardHarness } from '../test/clipboardHarness';

const submits = (h: ClipboardHarness) => h.fake.calls.filter((c) => c[0] === 'submit');

describe('transferTo', () => {
	it('copies, moves and links the selection into the folder, as selection sources', async () => {
		for (const kind of ['copy', 'move', 'link'] as const) {
			const h = await clipboardHarness();
			await selectIn(h.session, 1, 2);
			const done = h.commands.transferTo(kind, h.session, OTHER);
			await h.finish();
			await done;
			expect(h.lastRequest()).toEqual({
				kind: { kind },
				sources: {
					kind: 'selection',
					handle: h.session.model.handle,
					spec: { kind: 'some', ids: [1, 2] },
				},
				destination: OTHER,
				name: null,
				options: { conflict: null, verify: null },
				originWindow: 'main-1',
			});
		}
	});

	it('makes a duplicate of a copy into the folder the items are in, and a link there is still a link', async () => {
		const copy = await clipboardHarness();
		await selectIn(copy.session, 0);
		const done = copy.commands.transferTo('copy', copy.session, FOLDER);
		await copy.finish();
		await done;
		expect(copy.lastRequest()).toMatchObject({ kind: { kind: 'duplicate' }, destination: null });

		const link = await clipboardHarness();
		await selectIn(link.session, 0);
		const linked = link.commands.transferTo('link', link.session, FOLDER);
		await link.finish();
		await linked;
		expect(link.lastRequest()).toMatchObject({ kind: { kind: 'link' }, destination: FOLDER });
	});

	it('refuses a move into the folder the items are in, and says so', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 0);
		await h.commands.transferTo('move', h.session, FOLDER);
		expect(h.said.at(-1)).toBe('The items are already in that folder');
		expect(submits(h)).toHaveLength(0);
	});

	it('refuses a move out of a read-only location but copies and links from it', async () => {
		const h = await clipboardHarness({ readOnly: true });
		await selectIn(h.session, 0);
		await h.commands.transferTo('move', h.session, OTHER);
		expect(h.said.at(-1)).toBe('This location cannot be changed.');
		expect(submits(h)).toHaveLength(0);
		const copy = h.commands.transferTo('copy', h.session, OTHER);
		await h.finish();
		await copy;
		expect(submits(h)).toHaveLength(1);
	});

	it('says there is nothing to do when nothing is selected', async () => {
		const h = await clipboardHarness();
		await h.commands.transferTo('copy', h.session, OTHER);
		expect(h.said.at(-1)).toBe('Nothing is selected.');
		expect(submits(h)).toHaveLength(0);
	});
});

describe('Link To…', () => {
	it('asks for a folder, naming the count, then makes the links there', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 0, 1);
		h.pick.value = OTHER;
		const done = h.commands.linkTo();
		await h.finish();
		await done;
		expect(h.asked[0]).toMatchObject({
			title: 'Link 2 items in…',
			confirmLabel: 'Link',
			forbidOrigin: false,
		});
		expect(h.lastRequest()).toMatchObject({ kind: { kind: 'link' }, destination: OTHER });
	});

	it('does nothing when the dialog is cancelled', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 0);
		h.pick.value = null;
		await h.commands.linkTo();
		expect(submits(h)).toHaveLength(0);
	});
});

describe('transferLocations', () => {
	const files = [
		{ display: '/srv/with space.txt', uri: 'file:///srv/with%20space.txt' },
		{ display: '/srv/bad\ufffd.txt', uri: 'file:///srv/bad%FF.txt' },
	];

	it('copies and moves files from outside as location sources, keeping their lossless URIs', async () => {
		for (const [kind, job] of [
			['copy', 'copy'],
			['move', 'move'],
		] as const) {
			const h = await clipboardHarness();
			const done = h.commands.transferLocations(kind, files, OTHER);
			await h.finish();
			await done;
			expect(h.lastRequest()).toEqual({
				kind: { kind: job },
				sources: { kind: 'locations', locations: files },
				destination: OTHER,
				name: null,
				options: { conflict: null, verify: null },
				originWindow: 'main-1',
			});
		}
	});

	it('links them, which only the engine can refuse', async () => {
		const h = await clipboardHarness();
		const done = h.commands.transferLocations('link', files, OTHER);
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({
			kind: { kind: 'link' },
			sources: { kind: 'locations', locations: files },
			destination: OTHER,
		});
	});

	it('makes a duplicate of a copy into the folder the files are in', async () => {
		const h = await clipboardHarness();
		const here = [{ display: '/home/test/a.txt', uri: 'file:///home/test/a.txt' }];
		const done = h.commands.transferLocations('copy', here, FOLDER);
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({ kind: { kind: 'duplicate' }, destination: null });
	});

	it('refuses a folder dropped into itself, and a move onto the folder the files are in', async () => {
		const h = await clipboardHarness();
		await h.commands.transferLocations(
			'copy',
			[{ display: '/home/test', uri: 'file:///home/test' }],
			{ display: '/home/test/docs', uri: 'file:///home/test/docs' },
		);
		expect(h.said.at(-1)).toBe('A folder cannot be put inside itself');
		await h.commands.transferLocations(
			'move',
			[{ display: '/home/test/a.txt', uri: 'file:///home/test/a.txt' }],
			FOLDER,
		);
		expect(h.said.at(-1)).toBe('The items are already in that folder');
		expect(submits(h)).toHaveLength(0);
	});

	it('does nothing for no files', async () => {
		const h = await clipboardHarness();
		await h.commands.transferLocations('copy', [], OTHER);
		expect(submits(h)).toHaveLength(0);
	});
});
