// Verifies files dragged in from other applications and windows: the same targets, pill, spring-loading, default rule and drop as an in-page drag
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import {
	DOCS,
	disposeHarnesses,
	mark,
	MUSIC,
	nativeHarness,
	OUTSIDE,
	settle,
	type NativeHarness,
} from '../test/nativeDragHarness';
import { FOLDER } from '../test/browseHarness';
import { FILE_DRAG_ATTRIBUTE, PLAN_REST_MS } from './fileDrag';
import { folderRef } from './dropTargets';

afterEach(() => {
	disposeHarnesses();
	document.body.innerHTML = '';
	document.documentElement.removeAttribute('style');
	document.documentElement.removeAttribute(FILE_DRAG_ATTRIBUTE);
});

/** Lets the planner's answer arrive, as a pointer resting on a target does. */
async function rest(h: NativeHarness) {
	h.clock.advance(PLAN_REST_MS);
	await settle();
}

describe('entering', () => {
	it('shows what is dragged, by name for one file and by count for several', async () => {
		const h = await nativeHarness();
		h.enter([OUTSIDE[0]!]);
		expect(h.phase()).toBe('dragging');
		expect(h.pill()).toBe('Dragging with space.txt');
		expect(h.announced).toContain('Dragging with space.txt');
		expect(document.documentElement.getAttribute(FILE_DRAG_ATTRIBUTE)).toBe('idle');
		h.drag.session.cancel();
		h.enter(OUTSIDE);
		expect(h.pill()).toBe('Dragging 2 items');
	});

	it('refuses a second drag while one runs, and a window with no queue', async () => {
		const h = await nativeHarness();
		h.enter();
		expect(
			h.drag.beginNative({ files: OUTSIDE, point: { x: 1, y: 1 }, modifiers: noKeys }),
		).toBeNull();
		const none = await nativeHarness({ windowLabel: null });
		expect(
			none.drag.beginNative({ files: OUTSIDE, point: { x: 1, y: 1 }, modifiers: noKeys }),
		).toBeNull();
		expect(
			none.drag.beginNative({ files: [], point: { x: 1, y: 1 }, modifiers: noKeys }),
		).toBeNull();
	});

	it('moves the ghost with the positions it is given', async () => {
		const h = await nativeHarness();
		const feed = h.enter(OUTSIDE, { x: 20, y: 30 });
		expect(document.documentElement.style.getPropertyValue('--wp-drag-x')).toBe('20px');
		feed.move({ x: 300, y: 210 }, noKeys);
		expect(document.documentElement.style.getPropertyValue('--wp-drag-x')).toBe('300px');
		expect(document.documentElement.style.getPropertyValue('--wp-drag-y')).toBe('210px');
	});
});

const noKeys = { ctrl: false, shift: false, alt: false };

describe('a folder row', () => {
	it('guesses a copy until the planner says the volume, then moves on the same volume', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		h.over(mark('folder', folderRef(h.session.model.handle, 1), 'docs'));
		feed.move({ x: 140, y: 100 }, noKeys);
		await settle();
		expect(h.pill()).toBe('Move or copy 2 items to docs');
		await rest(h);
		expect(h.pill()).toBe('Move 2 items to docs');
		expect(h.plans).toHaveLength(1);
		expect(h.plans[0]).toMatchObject({
			kind: { kind: 'copy' },
			sources: { kind: 'locations', locations: OUTSIDE },
			destination: { uri: DOCS.uri },
			originWindow: 'main-1',
		});
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.transfers).toEqual([
			{ kind: 'move', items: OUTSIDE, destination: expect.objectContaining({ uri: DOCS.uri }) },
		]);
		expect(h.phase()).not.toBe('dragging');
		expect(document.documentElement.hasAttribute(FILE_DRAG_ATTRIBUTE)).toBe(false);
	});

	it('copies across volumes, and a Ctrl held at the drop copies on the same volume', async () => {
		const h = await nativeHarness();
		h.state.sameVolume = false;
		const feed = h.enter();
		h.over(mark('folder', folderRef(h.session.model.handle, 1), 'docs'));
		feed.move({ x: 140, y: 100 }, noKeys);
		await settle();
		await rest(h);
		expect(h.pill()).toBe('Copy 2 items to docs');
		h.state.sameVolume = true;
		feed.move({ x: 141, y: 100 }, { ctrl: true, shift: false, alt: false });
		expect(h.pill()).toBe('Copy 2 items to docs');
		await feed.drop({ x: 141, y: 100 }, { ctrl: true, shift: false, alt: false }, false);
		await settle();
		expect(h.transfers[0]?.kind).toBe('copy');
	});

	it('moves with Shift and links with Ctrl+Shift, read as the keys are at the drop', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		h.over(mark('place', MUSIC.uri, 'Music'));
		feed.move({ x: 140, y: 100 }, { ctrl: false, shift: true, alt: false });
		expect(h.target()?.outcome).toBe('move');
		feed.move({ x: 141, y: 100 }, { ctrl: true, shift: true, alt: false });
		expect(h.target()?.outcome).toBe('link');
		await feed.drop({ x: 141, y: 100 }, { ctrl: true, shift: true, alt: false }, false);
		await settle();
		expect(h.transfers[0]).toMatchObject({ kind: 'link', destination: { uri: MUSIC.uri } });
	});

	it('refuses a folder row that is one of the dragged items', async () => {
		const h = await nativeHarness();
		const feed = h.enter([{ display: '/home/test/docs', uri: DOCS.uri }]);
		h.over(mark('folder', folderRef(h.session.model.handle, 1), 'docs'));
		feed.move({ x: 140, y: 100 }, noKeys);
		await settle();
		expect(h.target()?.blocked).toEqual({ kind: 'source' });
		expect(h.pill()).toBe('Not allowed: docs is being dragged');
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.say).toEqual(['Docs is being dragged']);
		expect(h.transfers).toEqual([]);
	});

	it('refuses a read-only folder, and says why', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		h.over(mark('folder', '9:9', 'archive', { readOnly: true }));
		feed.move({ x: 140, y: 100 }, noKeys);
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.say).toEqual(['Archive cannot be changed']);
		expect(h.transfers).toEqual([]);
	});
});

describe('the other kinds of target', () => {
	it('drops into the pane, a breadcrumb and a sidebar place', async () => {
		for (const [kind, ref, label, uri] of [
			['pane', 1, 'test', FOLDER.uri],
			['crumb', DOCS.uri, 'docs', DOCS.uri],
			['place', MUSIC.uri, 'Music', MUSIC.uri],
		] as const) {
			const h = await nativeHarness();
			const feed = h.enter();
			h.over(mark(kind, ref, label));
			feed.move({ x: 140, y: 100 }, { ctrl: true, shift: false, alt: false });
			await settle();
			await feed.drop({ x: 140, y: 100 }, { ctrl: true, shift: false, alt: false }, false);
			await settle();
			expect(h.transfers, kind).toEqual([
				{ kind: 'copy', items: OUTSIDE, destination: expect.objectContaining({ uri }) },
			]);
			document.body.innerHTML = '';
		}
	});

	it('drops into a tab, which is that tab’s folder', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		h.over(mark('tab', 2, 'music'));
		feed.move({ x: 140, y: 100 }, noKeys);
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.transfers[0]?.destination.uri).toBe(MUSIC.uri);
	});

	it('opens tabs on + and a group chip, which write nothing', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		h.over(mark('plus', 'plus', 'New tab'));
		feed.move({ x: 140, y: 100 }, noKeys);
		expect(h.pill()).toBe('Open in new tabs');
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.opened).toHaveLength(1);
		expect(h.opened[0]).toMatchObject({ target: 'plus', group: null, split: false });
		expect(h.opened[0]!.source.external?.locations).toEqual(OUTSIDE);
		expect(h.transfers).toEqual([]);

		const chip = await nativeHarness();
		const second = chip.enter([OUTSIDE[0]!]);
		chip.over(mark('chip', 7, 'Work'));
		second.move({ x: 140, y: 100 }, noKeys);
		await second.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(chip.opened[0]).toMatchObject({ target: 'chip', group: 7 });
	});

	it('does not take the Trash: files from elsewhere are not trashed by a drop', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		h.over(mark('trash', 'trash:///', 'Trash'));
		feed.move({ x: 140, y: 100 }, noKeys);
		expect(h.target()?.blocked).toEqual({ kind: 'trashSource' });
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.say[0]).toContain('cannot be trashed from here');
		expect(h.transfers).toEqual([]);
	});

	it('has no target over the pane when the files are already in its folder', async () => {
		const h = await nativeHarness();
		const feed = h.enter([
			{ display: '/home/test/a.txt', uri: 'file:///home/test/a.txt' },
			{ display: '/home/test/b.txt', uri: 'file:///home/test/b.txt' },
		]);
		h.over(mark('pane', 1, 'test'));
		feed.move({ x: 140, y: 100 }, noKeys);
		expect(h.target()).toBeNull();
		// A move onto the folder they are in is refused; a copy there is a duplicate.
		h.over(mark('crumb', FOLDER.uri, 'test'));
		feed.move({ x: 141, y: 100 }, { ctrl: false, shift: true, alt: false });
		expect(h.target()?.blocked).toEqual({ kind: 'sameFolder' });
		feed.move({ x: 142, y: 100 }, { ctrl: true, shift: false, alt: false });
		expect(h.target()?.outcome).toBe('copy');
	});
});

describe('spring-loading', () => {
	it('opens a folder row after the delay, as an in-page drag does, and springs back on leaving', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		const pane = mark('pane', 1, 'test');
		pane.setAttribute('data-pane', '1');
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs', {}, pane);
		h.over(row, pane);
		feed.move({ x: 120, y: 100 }, noKeys);
		expect(row.hasAttribute('data-drop-spring')).toBe(true);
		h.clock.advance(599);
		expect(h.navigate).not.toHaveBeenCalled();
		h.clock.advance(1);
		await settle();
		expect(h.navigate).toHaveBeenCalledWith(1, expect.objectContaining({ uri: DOCS.uri }));
		expect(h.announced).toContain('Opened docs');
	});

	it('shows a tab after the delay', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		const tab = mark('tab', 2, 'music');
		h.over(tab);
		feed.move({ x: 120, y: 100 }, noKeys);
		h.clock.advance(600);
		await settle();
		expect(h.activate).toHaveBeenCalledWith(2);
		expect(h.announced).toContain('Showing music');
	});

	it('does not spring a target the pointer left first, nor after the files leave', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs');
		h.over(row);
		feed.move({ x: 120, y: 100 }, noKeys);
		h.clock.advance(300);
		h.over();
		feed.move({ x: 200, y: 100 }, noKeys);
		h.clock.advance(1000);
		await settle();
		expect(h.navigate).not.toHaveBeenCalled();
		h.over(row);
		feed.move({ x: 120, y: 100 }, noKeys);
		feed.leave();
		h.clock.advance(1000);
		await settle();
		expect(h.navigate).not.toHaveBeenCalled();
		expect(row.hasAttribute('data-drop-spring')).toBe(false);
	});
});

describe('where the modifier keys are not known (Wayland)', () => {
	it('applies the default rule, whatever the keys say', async () => {
		const h = await nativeHarness();
		h.state.sameVolume = false;
		const feed = h.enter();
		h.over(mark('place', MUSIC.uri, 'Music'));
		feed.move({ x: 140, y: 100 }, noKeys);
		await settle();
		await rest(h);
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.transfers[0]?.kind).toBe('copy');
	});

	it('opens the picker at the drop when the setting is to ask, and carries out the choice', async () => {
		const h = await nativeHarness();
		h.state.rule = 'alwaysAsk';
		const feed = h.enter();
		h.over(mark('place', MUSIC.uri, 'Music'));
		feed.move({ x: 140, y: 100 }, noKeys);
		expect(h.pill()).toBe('Choose what to do with 2 items in Music');
		await feed.drop({ x: 150, y: 110 }, noKeys, false);
		await settle();
		expect(h.pickers).toHaveLength(1);
		expect(h.pickers[0]).toMatchObject({
			position: { x: 150, y: 110 },
			what: '2 items',
			verbs: ['copy', 'move', 'link'],
		});
		expect(h.transfers).toEqual([]);
		h.pickers[0]!.choose('move');
		await settle();
		expect(h.transfers[0]).toMatchObject({ kind: 'move', destination: { uri: MUSIC.uri } });
	});

	it('offers no link in the picker for a remote destination', async () => {
		const h = await nativeHarness();
		h.state.rule = 'alwaysAsk';
		const feed = h.enter();
		h.over(mark('place', 'sftp://host/x', 'host'));
		feed.move({ x: 140, y: 100 }, noKeys);
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.pickers[0]?.verbs).toEqual(['copy', 'move']);
	});
});

describe('leaving and ending', () => {
	it('clears the highlight and the pill when the files leave, without a drop', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs');
		h.over(row);
		feed.move({ x: 140, y: 100 }, noKeys);
		await settle();
		expect(row.getAttribute('data-drop-over')).toBe('ok');
		feed.leave();
		expect(row.hasAttribute('data-drop-over')).toBe(false);
		expect(h.pill()).toBeUndefined();
		expect(h.phase()).not.toBe('dragging');
		expect(h.announced).toContain('The files left the window');
		expect(h.transfers).toEqual([]);
	});

	it('is ended by Esc where the page hears it, and a later release does nothing', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		h.over(mark('place', MUSIC.uri, 'Music'));
		feed.move({ x: 140, y: 100 }, noKeys);
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(h.announced).toContain('Drag cancelled');
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.transfers).toEqual([]);
	});

	it('says nothing was dropped when released over no target', async () => {
		const h = await nativeHarness();
		const feed = h.enter();
		feed.move({ x: 140, y: 100 }, noKeys);
		await feed.drop({ x: 140, y: 100 }, noKeys, false);
		await settle();
		expect(h.announced).toContain('Nothing was dropped');
		expect(h.transfers).toEqual([]);
	});

	it('lets a second drag begin once the first has settled', async () => {
		const h = await nativeHarness();
		h.enter().leave();
		expect(h.enter()).toBeDefined();
		expect(h.phase()).toBe('dragging');
	});
});

describe('file names', () => {
	it('keeps the lossless URIs of names with spaces and bytes that are not UTF-8', async () => {
		const h = await nativeHarness();
		const feed = h.enter(OUTSIDE);
		h.over(mark('place', MUSIC.uri, 'Music'));
		feed.move({ x: 140, y: 100 }, { ctrl: true, shift: false, alt: false });
		await feed.drop({ x: 140, y: 100 }, { ctrl: true, shift: false, alt: false }, false);
		await settle();
		expect(h.transfers[0]!.items.map((item) => item.uri)).toEqual([
			'file:///srv/share/with%20space.txt',
			'file:///srv/share/bad%FF%FE.txt',
		]);
	});
});
