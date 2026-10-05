// Verifies a sidebar place, or a folder dragged with Alt held, opens in a new pane when dropped on a split zone
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { openListingModel } from '../browse/listingModel';
import { selectIds } from '../browse/selection';
import { createListingSession } from '../browse/useListingSession';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import { FOLDER } from '../test/browseHarness';
import { dropAttributes } from './dropTargets';
import { createFileDrag, FILE_DRAG_ATTRIBUTE, type FileDragDeps } from './fileDrag';
import type { DragClock } from './dragSession';

const AREA = { left: 0, top: 0, right: 900, bottom: 600 };
const DOCS = fileLocation('/home/test/docs');
const OTHER = fileLocation('/home/test/other');
const noClock: DragClock = { setTimeout: () => 0, clearTimeout: () => {} };
const ENTRIES = [
	makeEntry(1, 'docs', { kind: 'directory' }),
	makeEntry(2, 'music', { kind: 'directory' }),
	makeEntry(3, 'notes.txt'),
];

async function setup(canSplit = true) {
	const vfs = new FakeVfsClient();
	vfs.setFolder(FOLDER, ENTRIES);
	const session = createListingSession(await openListingModel(vfs, FOLDER));
	const state = { canSplit };
	const stack: Element[] = [];
	const announced: string[] = [];
	const said: string[] = [];
	const deps = {
		rule: () => 'byVolume',
		springMs: () => 600,
		announce: (text: string) => void announced.push(text),
		say: (text: string) => void said.push(text),
		windowLabel: () => 'main-1',
		plan: vi.fn(async () => {
			throw new Error('not asked');
		}),
		entryLocation: vi.fn((handle: number, entry: number) => vfs.entryLocation(handle, entry)),
		pane: () => ({ location: OTHER, readOnly: false }),
		tabLocation: () => FOLDER,
		activeTab: () => 1,
		navigate: vi.fn(async () => {}),
		back: vi.fn(async () => {}),
		activate: vi.fn(async () => {}),
		retain: () => () => {},
		transfer: vi.fn(async () => {}),
		moveToTrash: vi.fn(async () => {}),
		openFolders: vi.fn(async () => {}),
		openPicker: vi.fn(),
		trashAvailable: () => true,
		area: () => AREA,
		canSplit: () => state.canSplit,
		openInSplit: vi.fn(async () => {}),
		hit: { elementsFromPoint: () => stack },
		clock: noClock,
		reducedMotion: () => true,
	};
	const drag = createFileDrag(deps as unknown as FileDragDeps);
	const element = document.createElement('div');
	element.setPointerCapture = vi.fn();
	element.releasePointerCapture = vi.fn();
	document.body.append(element);
	const pane = document.createElement('div');
	for (const [name, value] of Object.entries(dropAttributes('pane', 1, 'docs'))) {
		pane.setAttribute(name, value);
	}
	document.body.append(pane);
	const none = { ctrl: false, shift: false, alt: false };
	const h = {
		drag,
		deps,
		state,
		announced,
		said,
		pane,
		stack,
		target: () => drag.session.store.getState().target,
		pill: () => drag.session.store.getState().pill?.text,
		pressPlace(label = 'Documents') {
			expect(
				drag.pressLocations({
					pointerId: 1,
					clientX: 10,
					clientY: 10,
					button: 0,
					element,
					locations: [DOCS],
					name: label,
					groups: ['folder'],
					folder: null,
					modifiers: none,
					place: true,
				}),
			).toBe(true);
		},
		pressRow(position: number, over: { button?: number } = {}) {
			expect(
				drag.press({
					pointerId: 1,
					clientX: 10,
					clientY: 10,
					button: over.button ?? 0,
					element,
					session,
					position,
					entry: ENTRIES[position]!,
					tab: 1,
					modifiers: none,
				}),
			).toBe(true);
		},
		move(x: number, y: number, keys: { altKey?: boolean } = {}) {
			fireEvent.pointerMove(window, { clientX: x, clientY: y, pointerId: 1, ...keys });
		},
		up(x: number, y: number, keys: { altKey?: boolean } = {}) {
			fireEvent.pointerUp(window, { clientX: x, clientY: y, pointerId: 1, ...keys });
		},
		session,
	};
	return h;
}

afterEach(() => {
	document.body.innerHTML = '';
	document.documentElement.removeAttribute(FILE_DRAG_ATTRIBUTE);
});

const settle = async () => {
	for (let i = 0; i < 8; i++) await Promise.resolve();
};

describe('dragging a sidebar place', () => {
	it('shows the split zone under the pointer, with what will happen', async () => {
		const h = await setup();
		h.pressPlace();
		h.move(30, 300);
		h.stack.splice(0, 0, h.pane);
		h.move(100, 300);
		expect(h.target()).toMatchObject({ outcome: 'split', edge: 'left' });
		expect(h.pill()).toBe('Open Documents in a new left pane');
		h.move(800, 300);
		expect(h.target()).toMatchObject({ outcome: 'split', edge: 'right' });
		h.move(450, 100);
		expect(h.target()).toMatchObject({ outcome: 'split', edge: 'top' });
		expect(h.pill()).toBe('Open Documents in a new top pane');
		h.move(450, 500);
		expect(h.target()).toMatchObject({ outcome: 'split', edge: 'bottom' });
		expect(h.announced.at(-1)).toBe(
			'Over the bottom split zone: will open Documents in a new pane',
		);
	});

	it('opens the place in a new pane on the zone it is released on', async () => {
		const h = await setup();
		h.pressPlace();
		h.move(30, 300);
		h.stack.splice(0, 0, h.pane);
		h.move(100, 300);
		h.up(100, 300);
		await settle();
		expect(h.deps.openInSplit).toHaveBeenCalledWith(DOCS, 'left');
		expect(h.deps.transfer).not.toHaveBeenCalled();
	});

	it('opens it even when the pointer is over a divider, which is no drop target', async () => {
		const h = await setup();
		h.pressPlace();
		h.move(30, 300);
		h.stack.splice(0, h.stack.length);
		h.move(800, 300);
		expect(h.target()).toMatchObject({ outcome: 'split', edge: 'right' });
		h.up(800, 300);
		await settle();
		expect(h.deps.openInSplit).toHaveBeenCalledWith(DOCS, 'right');
	});

	it('does nothing away from the file area, and says nothing is dropped', async () => {
		const h = await setup();
		h.pressPlace();
		h.move(30, 300);
		h.move(1200, 300);
		expect(h.target()).toBeNull();
		expect(h.pill()).toBe('Dragging Documents');
		h.up(1200, 300);
		await settle();
		expect(h.deps.openInSplit).not.toHaveBeenCalled();
		expect(h.announced).toContain('Nothing was dropped');
	});

	it('refuses over a tab that is already split, and says why on release', async () => {
		const h = await setup(false);
		h.pressPlace();
		h.move(30, 300);
		h.move(100, 300);
		expect(h.target()).toMatchObject({ outcome: null, blocked: { kind: 'alreadySplit' } });
		expect(h.pill()).toBe('Not allowed: this tab is already split');
		h.up(100, 300);
		await settle();
		expect(h.deps.openInSplit).not.toHaveBeenCalled();
		expect(h.said).toEqual(['This tab is already split']);
	});
});

describe('dragging a folder from the file view', () => {
	const into = async (position = 0, canSplit = true) => {
		const h = await setup(canSplit);
		h.pressRow(position);
		h.move(30, 300);
		h.stack.splice(0, 0, h.pane);
		return { h };
	};

	it('keeps its meaning over a pane without the modifier', async () => {
		const { h } = await into();
		h.move(100, 300);
		expect(h.target()).toMatchObject({ kind: 'pane' });
		expect(h.target()?.outcome).not.toBe('split');
	});

	it('opens in a new pane with Alt held, and the pill says so', async () => {
		const { h } = await into();
		h.move(100, 300, { altKey: true });
		expect(h.target()).toMatchObject({ outcome: 'split', edge: 'left' });
		expect(h.pill()).toBe('Open docs in a new left pane');
		// Letting go of Alt goes back to the drop it was.
		h.move(100, 300, { altKey: false });
		expect(h.target()?.outcome).not.toBe('split');
		h.move(100, 300, { altKey: true });
		h.up(100, 300, { altKey: true });
		await settle();
		expect(h.deps.openInSplit).toHaveBeenCalledWith(
			expect.objectContaining({ uri: expect.stringContaining('docs') }),
			'left',
		);
		expect(h.deps.transfer).not.toHaveBeenCalled();
	});

	it('is a plain Alt drop (the picker) for a file, which has no folder to open', async () => {
		const { h } = await into(2);
		h.move(100, 300, { altKey: true });
		expect(h.target()?.outcome).toBe('ask');
	});

	it('is the picker for a right-button drag, which always asks', async () => {
		const h = await setup();
		h.pressRow(0, { button: 2 });
		h.move(30, 300);
		h.stack.splice(0, 0, h.pane);
		h.move(100, 300, { altKey: true });
		expect(h.target()?.outcome).toBe('ask');
	});

	it('falls back to the picker while the tab is already split, since the zones are not offered', async () => {
		const { h } = await into(0, false);
		h.move(100, 300, { altKey: true });
		expect(h.target()?.outcome).toBe('ask');
	});

	it('does not split a drag of more than one item', async () => {
		const h = await setup();
		h.session.store.setState({ selection: selectIds([1, 2]) });
		h.pressRow(0);
		h.move(30, 300);
		h.stack.splice(0, 0, h.pane);
		h.move(100, 300, { altKey: true });
		expect(h.target()?.outcome).not.toBe('split');
	});
});
