// Verifies a file drag that leaves the window: the hand-over to the system's drag, the fallbacks, and how its end is reported
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { fileLocation, makeEntry } from '../services/fakeVfsClient';
import type { DragEnded } from '../services/nativeDndClient';
import {
	disposeHarnesses,
	mark,
	nativeHarness,
	OWN,
	settle,
	type NativeHarness,
} from '../test/nativeDragHarness';
import { FILE_DRAG_ATTRIBUTE, OWN_DRAG_MAX_MS, OWN_DRAG_MS, PREFETCH_LIMIT } from './fileDrag';
import { folderRef } from './dropTargets';

afterEach(() => {
	disposeHarnesses();
	document.body.innerHTML = '';
	document.documentElement.removeAttribute('style');
	document.documentElement.removeAttribute(FILE_DRAG_ATTRIBUTE);
});

const noKeys = { ctrl: false, shift: false, alt: false };

const ended = (id: number, outcome: DragEnded['outcome'], reason: string | null = null) =>
	({ id, outcome, uris: [], reason }) satisfies DragEnded;

/** Drags a row and takes the pointer out of the window. */
async function leave(h: NativeHarness, position = 2) {
	h.startDrag(position);
	h.move(-5);
	await settle();
}

describe('leaving the window', () => {
	it('hands the drag to the system with the selection’s URIs and the actions it allows', async () => {
		const h = await nativeHarness();
		h.state.resolve = async () => [
			fileLocation('/home/test/notes.txt'),
			fileLocation('/home/test/with space.txt'),
		];
		await leave(h);
		expect(h.started).toEqual([
			{
				uris: ['file:///home/test/notes.txt', 'file:///home/test/with%20space.txt'],
				actions: ['copy', 'move', 'link'],
			},
		]);
		// The in-page drag is over, quietly: nothing is marked and the pill is gone.
		expect(h.phase()).not.toBe('dragging');
		expect(h.pill()).toBeUndefined();
		expect(h.announced).toContain('Dragging notes.txt out of the window');
		expect(h.announced).not.toContain('Drag cancelled');
		expect(document.documentElement.hasAttribute(FILE_DRAG_ATTRIBUTE)).toBe(false);
		expect(h.say).toEqual([]);
	});

	it('offers no move from a folder that cannot be written to', async () => {
		const h = await nativeHarness({ readOnly: true });
		await leave(h);
		expect(h.started[0]?.actions).toEqual(['copy', 'link']);
	});

	it('does not offer a link for items that are not local', async () => {
		const h = await nativeHarness({ folder: { display: 'host', uri: 'sftp://host/home' } });
		await leave(h);
		expect(h.started[0]?.actions).toEqual(['copy', 'move']);
	});

	it('downloads a server’s files first and hands the copies over, as copies only', async () => {
		const h = await nativeHarness({ folder: { display: 'host', uri: 'sftp://host/home' } });
		const remote = { display: 'sftp://host/home/notes.txt', uri: 'sftp://host/home/notes.txt' };
		h.state.resolve = async () => [remote];
		const staged: Location[][] = [];
		h.state.stage = async (locations) => {
			staged.push(locations);
			return [fileLocation('/cache/drag-out/notes.txt')];
		};
		await leave(h);
		expect(staged).toEqual([[remote]]);
		expect(h.started).toEqual([{ uris: ['file:///cache/drag-out/notes.txt'], actions: ['copy'] }]);
		expect(h.announced).toContain('Downloading notes.txt for the drag');
	});

	it('keeps a drag of a server’s files in the page, and says why, when they cannot be downloaded', async () => {
		const h = await nativeHarness({ folder: { display: 'host', uri: 'sftp://host/home' } });
		h.state.resolve = async () => [{ display: 'x', uri: 'sftp://host/home/dir' }];
		h.state.stage = async () => {
			throw { kind: 'ops', message: 'no', error: { kind: 'unsupported', what: 'a folder' } };
		};
		await leave(h);
		expect(h.started).toEqual([]);
		expect(h.phase()).toBe('dragging');
		expect(h.say[0]).toMatch(/^Notes\.txt could not be downloaded for the drag/);
	});

	it('hands over when the pointer goes past any edge', async () => {
		for (const [x, y] of [
			[-1, 100],
			[800, 100],
			[100, -1],
			[100, 600],
		] as const) {
			const h = await nativeHarness();
			h.startDrag();
			h.move(x, y);
			await settle();
			expect(h.started, `${x},${y}`).toHaveLength(1);
		}
	});

	it('asks again only once, however many moves follow while the hand-over is under way', async () => {
		const h = await nativeHarness();
		h.startDrag();
		h.move(-5);
		h.move(-9);
		h.move(-12);
		await settle();
		expect(h.started).toHaveLength(1);
	});

	it('hands over when the document reports the pointer left, with no coordinates outside', async () => {
		const h = await nativeHarness();
		h.startDrag();
		fireEvent.pointerLeave(document.documentElement);
		await settle();
		expect(h.started).toHaveLength(1);
	});

	it('does not hand a drag made with the right button to the system', async () => {
		const h = await nativeHarness();
		h.startDrag(2, { button: 2 });
		h.move(-5);
		await settle();
		expect(h.started).toEqual([]);
		expect(h.phase()).toBe('dragging');
	});

	it('resolves the locations ahead for a small selection, and when the pointer leaves for a large one', async () => {
		const small = await nativeHarness();
		let asked = 0;
		small.state.resolve = async () => {
			asked++;
			return [fileLocation('/home/test/notes.txt')];
		};
		small.startDrag();
		expect(asked).toBe(1);

		const big = await nativeHarness({
			entries: Array.from({ length: PREFETCH_LIMIT + 5 }, (_, id) =>
				makeEntry(id + 1, `f${id}.txt`),
			),
		});
		let askedBig = 0;
		big.state.resolve = async () => {
			askedBig++;
			return [fileLocation('/home/test/f0.txt')];
		};
		big.session.store.getState().selectAll();
		big.startDrag(0);
		expect(big.drag.session.store.getState().source!.count).toBe(PREFETCH_LIMIT + 5);
		expect(askedBig).toBe(0);
		big.move(-5);
		await settle();
		expect(askedBig).toBe(1);
		expect(big.started).toHaveLength(1);
	});
});

describe('where the drag stays in the page', () => {
	it('stays when outbound drags are unavailable, and leaving does nothing', async () => {
		const h = await nativeHarness({ outbound: false });
		h.startDrag();
		h.move(-5);
		await settle();
		expect(h.started).toEqual([]);
		expect(h.phase()).toBe('dragging');
		expect(h.say).toEqual([]);
	});

	it('stays with a notice when the system refuses, and tries again only after the pointer was back inside', async () => {
		const h = await nativeHarness();
		let refuse = true;
		h.state.start = async (request) => {
			if (refuse) throw { kind: 'buttonNotPressed', message: 'the primary button is not down' };
			h.started.push(request);
			return { id: 7, ended: null };
		};
		h.startDrag();
		h.move(-5);
		await settle();
		expect(h.say).toEqual(['Could not drag notes.txt out of the window, so the drag stays here']);
		expect(h.phase()).toBe('dragging');
		// The pill says what is dragged again.
		expect(h.pill()).toBe('Dragging notes.txt');
		// Still outside: no second try.
		h.move(-9);
		await settle();
		expect(h.say).toHaveLength(1);
		// Back inside and out again: another try, which now works.
		refuse = false;
		h.move(300);
		h.move(-5);
		await settle();
		expect(h.started).toHaveLength(1);
		expect(h.phase()).not.toBe('dragging');
	});

	it('asks for the keys to be released when a key during the press would make the system ignore the drag', async () => {
		const h = await nativeHarness();
		h.state.start = async () => {
			throw { kind: 'keysHeld', message: 'a key was pressed or released during the press' };
		};
		h.startDrag();
		h.move(-5);
		await settle();
		expect(h.say).toEqual([
			'Could not drag notes.txt out of the window because a key was pressed while the mouse button was down; release the keys and drag again',
		]);
		expect(h.phase()).toBe('dragging');
	});

	it('says what failed for any other refusal', async () => {
		const h = await nativeHarness();
		h.state.start = async () => {
			throw { kind: 'failed', message: 'no drag source' };
		};
		h.startDrag();
		h.move(-5);
		await settle();
		expect(h.say).toEqual(['The drag out of the window failed: no drag source']);
		expect(h.phase()).toBe('dragging');
	});

	it('stays with a notice when Rust cannot resolve the items, or they are not local', async () => {
		const h = await nativeHarness();
		h.state.resolve = async () => {
			throw new Error('gone');
		};
		h.startDrag();
		h.move(-5);
		await settle();
		expect(h.say).toEqual(['Could not drag notes.txt out of the window, so the drag stays here']);
		const remote = await nativeHarness();
		remote.state.resolve = async () => [{ display: 'x', uri: 'sftp://host/x' }];
		remote.startDrag();
		remote.move(-5);
		await settle();
		expect(remote.say).toEqual(['Notes.txt cannot be dragged out of the window']);
		expect(remote.started).toEqual([]);
	});

	it('does nothing when the drag was released while the locations were coming', async () => {
		const h = await nativeHarness();
		let resolve!: (locations: ReturnType<typeof fileLocation>[]) => void;
		h.state.resolve = () => new Promise((done) => (resolve = done));
		h.startDrag();
		// The prefetch is under way; the pointer leaves and the button is released.
		h.move(-5);
		h.up(-5);
		resolve([fileLocation('/home/test/notes.txt')]);
		await settle();
		expect(h.started).toEqual([]);
		expect(h.say).toEqual([]);
	});

	it('is cancelled by Esc before it leaves, and nothing is handed over afterwards', async () => {
		const h = await nativeHarness();
		h.startDrag();
		fireEvent.keyDown(window, { key: 'Escape' });
		h.move(-5);
		await settle();
		expect(h.started).toEqual([]);
		expect(h.announced).toContain('Drag cancelled');
	});
});

describe('how the system drag ends', () => {
	const cases: Array<[DragEnded['outcome'], string]> = [
		['dropped-copy', 'Dropped notes.txt in another application'],
		['dropped-move', 'Moved notes.txt to another application'],
		['dropped-link', 'Linked notes.txt in another application'],
		['cancelled', 'Drag cancelled'],
	];
	for (const [outcome, said] of cases) {
		it(`announces ${outcome}, and runs no job here`, async () => {
			const h = await nativeHarness();
			await leave(h);
			h.drag.dragEnded(ended(1, outcome));
			expect(h.announced.at(-1)).toBe(said);
			expect(h.moved).toEqual([]);
			expect(h.transfers).toEqual([]);
			expect(h.plans).toEqual([]);
		});
	}

	it('says so when the drag failed', async () => {
		const h = await nativeHarness();
		await leave(h);
		h.drag.dragEnded(ended(1, 'failed', 'the compositor refused'));
		expect(h.say).toEqual(['The drag out of the window failed: the compositor refused']);
	});

	it('ignores the end of a drag it did not start', async () => {
		const h = await nativeHarness();
		h.drag.dragEnded(ended(9, 'dropped-copy'));
		await leave(h);
		h.drag.dragEnded(ended(9, 'dropped-copy'));
		expect(h.announced.at(-1)).toBe('Dragging notes.txt out of the window');
	});

	it('takes the end that arrives before the start command has answered', async () => {
		const h = await nativeHarness();
		let answer!: (started: { id: number; ended: null }) => void;
		h.state.start = () => new Promise((done) => (answer = done));
		h.startDrag();
		h.move(-5);
		await settle();
		h.drag.dragEnded(ended(4, 'dropped-move'));
		answer({ id: 4, ended: null });
		await settle();
		expect(h.announced).toContain('Moved notes.txt to another application');
	});

	it('takes the end the start command carries, where it only answers when the drag is over (Windows)', async () => {
		const h = await nativeHarness();
		h.state.start = async () => ({ id: 3, ended: ended(3, 'dropped-copy') });
		await leave(h);
		expect(h.announced.at(-1)).toBe('Dropped notes.txt in another application');
		// The plugin sends the same end as an event too: it is said once.
		const said = h.announced.length;
		h.drag.dragEnded(ended(3, 'dropped-copy'));
		expect(h.announced).toHaveLength(said);
	});

	it('lets another drag start straight away', async () => {
		const h = await nativeHarness();
		await leave(h);
		h.drag.dragEnded(ended(1, 'cancelled'));
		h.state.start = async (request) => {
			h.started.push(request);
			return { id: 2, ended: null };
		};
		await leave(h, 3);
		expect(h.started).toHaveLength(2);
	});
});

describe('the drag coming back', () => {
	it('keeps what the drag was: the folder it came from, and that it cannot be moved from a read-only one', async () => {
		const h = await nativeHarness({ readOnly: true });
		await leave(h);
		const feed = h.enter(OWN);
		const source = h.drag.session.store.getState().source!;
		expect(source.external?.own).toBe(true);
		expect(source.readOnly).toBe(true);
		expect(source.name).toBe('notes.txt');
		// Over the pane it came from, there is nothing to drop on.
		h.over(mark('pane', 1, 'test'));
		feed.move({ x: 140, y: 100 }, noKeys);
		expect(h.target()).toBeNull();
		// A move onto another folder becomes a copy: the folder cannot be written to.
		h.over(mark('place', 'file:///home/test/music', 'Music'));
		feed.move({ x: 141, y: 100 }, { ctrl: false, shift: true, alt: false });
		expect(h.target()?.outcome).toBe('copy');
	});

	it('refuses to drop a dragged folder on its own row', async () => {
		const h = await nativeHarness();
		h.state.resolve = async () => [fileLocation('/home/test/docs')];
		await leave(h, 0);
		const feed = h.enter([{ display: '/home/test/docs', uri: 'file:///home/test/docs' }]);
		h.over(mark('folder', folderRef(h.session.model.handle, 1), 'docs'));
		feed.move({ x: 140, y: 100 }, noKeys);
		await settle();
		expect(h.target()?.blocked).toEqual({ kind: 'source' });
	});

	it('says nothing at the end of the drag when it was dropped here, since the job says it', async () => {
		const h = await nativeHarness();
		await leave(h);
		const feed = h.enter(OWN);
		h.over(mark('place', 'file:///home/test/music', 'Music'));
		feed.move({ x: 140, y: 100 }, { ctrl: true, shift: false, alt: false });
		await feed.drop({ x: 140, y: 100 }, { ctrl: true, shift: false, alt: false }, true);
		await settle();
		expect(h.transfers).toHaveLength(1);
		const before = h.announced.length;
		h.drag.dragEnded(ended(1, 'dropped-copy'));
		expect(h.announced).toHaveLength(before);
	});

	it('treats the same files, a moment after the drag has ended, as its own, and forgets them later', async () => {
		const h = await nativeHarness({ readOnly: true });
		await leave(h);
		h.drag.dragEnded(ended(1, 'dropped-copy'));
		h.enter(OWN).leave();
		// Back as an own drag within the window.
		h.enter(OWN);
		expect(h.drag.session.store.getState().source!.external?.own).toBe(true);
		h.drag.session.cancel();
		h.clock.advance(OWN_DRAG_MS + 1);
		h.enter(OWN);
		expect(h.drag.session.store.getState().source!.external?.own).toBe(false);
	});

	it('stops remembering its own drag when the end never arrives, so the same files from elsewhere are not mistaken for it', async () => {
		const h = await nativeHarness({ readOnly: true });
		await leave(h);
		// No `drag-ended` comes. A long hold is still its own drag.
		h.clock.advance(OWN_DRAG_MAX_MS - 1000);
		h.enter(OWN);
		expect(h.drag.session.store.getState().source!.external?.own).toBe(true);
		h.drag.session.cancel();
		h.clock.advance(1001);
		h.enter(OWN);
		const source = h.drag.session.store.getState().source!;
		expect(source.external?.own).toBe(false);
		expect(source.readOnly).toBe(false);
	});

	it('is files from elsewhere when the files are not the ones that were offered', async () => {
		const h = await nativeHarness({ readOnly: true });
		await leave(h);
		h.enter([{ display: '/home/test/photo.jpg', uri: 'file:///home/test/photo.jpg' }]);
		expect(h.drag.session.store.getState().source!.external?.own).toBe(false);
		expect(h.drag.session.store.getState().source!.readOnly).toBe(false);
	});
});

describe('tab drags', () => {
	it('is never started by a file drag leaving the window, and a file drag does not start beside a running one', async () => {
		const h = await nativeHarness();
		await leave(h);
		expect(h.phase()).not.toBe('dragging');
		// The hand-off asked only the system; no tear-off client is in play at this level.
		expect(h.started).toHaveLength(1);
	});
});
