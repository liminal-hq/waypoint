// Verifies the file drag: thresholds, selection semantics, targets, the default rule, modifiers, springs, scrolling and the drop
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { fireEvent } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { openListingModel } from '../browse/listingModel';
import { createListingSession, type ListingSession } from '../browse/useListingSession';
import { selectOnly } from '../browse/selection';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import type { PlanPreview } from '../services/opsClient';
import { FOLDER } from '../test/browseHarness';
import type { DragClock } from './dragSession';
import { dropAttributes, folderRef, SCROLL_ATTRIBUTE } from './dropTargets';
import {
	createFileDrag,
	FILE_DRAG_ATTRIBUTE,
	PLAN_REST_MS,
	SPRING_JITTER_PX,
	type FileDrag,
	type FileDragDeps,
	type FileDragPress,
	type OpenFoldersRequest,
	type PickerRequest,
} from './fileDrag';
import type { FileDropTarget } from './fileDragModel';

/** A clock the test winds by hand, so holds and ticks need no waiting. */
class ManualClock implements DragClock {
	private now = 0;
	private next = 1;
	private timers = new Map<number, { at: number; handler: () => void }>();
	setTimeout(handler: () => void, ms: number) {
		const id = this.next++;
		this.timers.set(id, { at: this.now + ms, handler });
		return id;
	}
	clearTimeout(handle: unknown) {
		this.timers.delete(handle as number);
	}
	advance(ms: number) {
		const end = this.now + ms;
		for (;;) {
			const due = [...this.timers.entries()]
				.filter(([, timer]) => timer.at <= end)
				.sort((a, b) => a[1].at - b[1].at)[0];
			if (!due) break;
			this.timers.delete(due[0]);
			this.now = due[1].at;
			due[1].handler();
		}
		this.now = end;
	}
}

const DOCS = fileLocation('/home/test/docs');
const MUSIC = fileLocation('/home/test/music');
const ENTRIES = [
	makeEntry(1, 'docs', { kind: 'directory' }),
	makeEntry(2, 'music', { kind: 'directory' }),
	makeEntry(3, 'notes.txt'),
	makeEntry(4, 'photo.jpg'),
];

const planOf = (sameVolume: boolean): PlanPreview => ({
	kind: { kind: 'copy' },
	sources: { count: 1, first: 'a', bytes: 1 } as unknown as PlanPreview['sources'],
	items: 1,
	bytes: 1,
	sameVolume,
	ends: { from: [], to: null },
	conflicts: [],
	notes: [],
});

interface Harness {
	drag: FileDrag;
	session: ListingSession;
	clock: ManualClock;
	deps: {
		[K in keyof FileDragDeps]: FileDragDeps[K] extends (...args: infer A) => infer R
			? ReturnType<typeof vi.fn<(...args: A) => R>>
			: FileDragDeps[K];
	};
	/** What the hit test reports under the pointer, topmost first. */
	stack: Element[];
	say: string[];
	announced: string[];
	pickers: PickerRequest[];
	opened: OpenFoldersRequest[];
	released: ReturnType<typeof vi.fn>;
	state: { sameVolume: boolean; planError: unknown; rule: 'byVolume' | 'alwaysCopy' | 'alwaysAsk' };
	target(): FileDropTarget | null;
	pill(): string | undefined;
	press(position?: number, over?: Partial<FileDragPress>): boolean;
	move(
		x: number,
		y?: number,
		keys?: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean },
	): void;
	up(
		x?: number,
		y?: number,
		keys?: { ctrlKey?: boolean; shiftKey?: boolean; altKey?: boolean },
	): void;
	/** Presses on the first row and drags far enough to start. */
	startDrag(position?: number, over?: Partial<FileDragPress>): void;
	over(...elements: Element[]): void;
}

async function setup(
	options: {
		entries?: Entry[];
		readOnly?: boolean;
		trash?: boolean;
		windowLabel?: string | null;
		panes?: Record<number, { location: typeof DOCS; readOnly: boolean }>;
	} = {},
): Promise<Harness> {
	const vfs = new FakeVfsClient();
	vfs.setFolder(FOLDER, options.entries ?? ENTRIES);
	if (options.readOnly) vfs.setReadOnly(FOLDER);
	if (options.trash) vfs.markTrash(FOLDER);
	const session = createListingSession(await openListingModel(vfs, FOLDER));
	const clock = new ManualClock();
	const say: string[] = [];
	const announced: string[] = [];
	const pickers: PickerRequest[] = [];
	const opened: OpenFoldersRequest[] = [];
	const released = vi.fn();
	const state: Harness['state'] = { sameVolume: true, planError: null, rule: 'byVolume' };
	const stack: Element[] = [];
	const deps = {
		rule: vi.fn(() => state.rule),
		springMs: vi.fn(() => 600),
		announce: vi.fn((text: string) => void announced.push(text)),
		say: vi.fn((text: string) => void say.push(text)),
		windowLabel: vi.fn(() => (options.windowLabel === undefined ? 'main-1' : options.windowLabel)),
		plan: vi.fn(async () => {
			if (state.planError) throw state.planError;
			return planOf(state.sameVolume);
		}),
		entryLocation: vi.fn((handle: number, entry: number) => vfs.entryLocation(handle, entry)),
		pane: vi.fn((tab: number) => options.panes?.[tab] ?? { location: FOLDER, readOnly: false }),
		tabLocation: vi.fn((tab: number) => (tab === 2 ? MUSIC : tab === 1 ? FOLDER : null)),
		activeTab: vi.fn(() => 1),
		navigate: vi.fn(async () => {}),
		back: vi.fn(async () => {}),
		activate: vi.fn(async () => {}),
		retain: vi.fn(() => released),
		transfer: vi.fn(async () => {}),
		moveToTrash: vi.fn(async () => {}),
		openFolders: vi.fn(async (request: OpenFoldersRequest) => void opened.push(request)),
		openPicker: vi.fn((request: PickerRequest) => void pickers.push(request)),
		trashAvailable: vi.fn(() => true),
		hit: { elementsFromPoint: () => stack },
		clock,
		reducedMotion: () => true,
	} satisfies Record<string, unknown>;
	const drag = createFileDrag(deps as unknown as FileDragDeps);
	const element = document.createElement('div');
	document.body.append(element);
	element.setPointerCapture = vi.fn();
	element.releasePointerCapture = vi.fn();
	const harness: Harness = {
		drag,
		session,
		clock,
		deps: deps as unknown as Harness['deps'],
		stack,
		say,
		announced,
		pickers,
		opened,
		released,
		state,
		target: () => drag.session.store.getState().target,
		pill: () => drag.session.store.getState().pill?.text,
		press(position = 2, over = {}) {
			const entry = (options.entries ?? ENTRIES)[position]!;
			return drag.press({
				pointerId: 1,
				clientX: 100,
				clientY: 100,
				button: 0,
				element,
				session,
				position,
				entry,
				tab: 1,
				modifiers: { ctrl: false, shift: false, alt: false },
				...over,
			});
		},
		move(x, y = 100, keys = {}) {
			fireEvent.pointerMove(window, { clientX: x, clientY: y, pointerId: 1, ...keys });
		},
		up(x = 200, y = 100, keys = {}) {
			fireEvent.pointerUp(window, { clientX: x, clientY: y, pointerId: 1, ...keys });
		},
		startDrag(position = 2, over = {}) {
			expect(harness.press(position, over)).toBe(true);
			harness.move(110);
			expect(drag.session.store.getState().phase).toBe('dragging');
		},
		over(...elements) {
			stack.splice(0, stack.length, ...elements);
		},
	};
	return harness;
}

/** A marked element in the document. */
function mark(
	kind: Parameters<typeof dropAttributes>[0],
	ref: string | number,
	label: string,
	options: Parameters<typeof dropAttributes>[3] = {},
	parent: HTMLElement = document.body,
): HTMLElement {
	const element = document.createElement('div');
	for (const [name, value] of Object.entries(dropAttributes(kind, ref, label, options))) {
		element.setAttribute(name, value);
	}
	parent.append(element);
	return element;
}

afterEach(() => {
	document.body.innerHTML = '';
	document.documentElement.removeAttribute('style');
	document.documentElement.removeAttribute(FILE_DRAG_ATTRIBUTE);
});

const settle = async () => {
	for (let i = 0; i < 5; i++) await Promise.resolve();
};

describe('the press', () => {
	it('is not a drag until the pointer has moved 4 px, and a release before that is a click', async () => {
		const h = await setup();
		expect(h.press()).toBe(true);
		h.move(102);
		expect(h.drag.session.store.getState().phase).toBe('pending');
		h.up(102);
		expect(h.drag.session.store.getState().phase).toBe('idle');
		expect(h.deps.retain).not.toHaveBeenCalled();
		expect(h.session.store.getState().selection).toEqual({ kind: 'some', ids: new Set() });
		h.startDrag();
		expect(h.drag.session.store.getState().phase).toBe('dragging');
	});

	it('selects an unselected row when the drag starts, not when it is pressed', async () => {
		const h = await setup();
		h.press(2);
		expect(h.session.store.getState().selection.ids.size).toBe(0);
		h.move(110);
		expect(h.session.store.getState().selection).toEqual(selectOnly(3));
		expect(h.drag.session.store.getState().source).toMatchObject({
			count: 1,
			name: 'notes.txt',
			spec: { kind: 'some', ids: [3] },
		});
	});

	it('drags the whole selection when a selected row is pressed', async () => {
		const h = await setup();
		h.session.store.getState().selectEntries([1, 3, 4], 2);
		h.startDrag(2);
		const source = h.drag.session.store.getState().source!;
		expect(source.count).toBe(3);
		expect(source.name).toBeNull();
		expect(source.groups).toHaveLength(3);
		expect(h.session.store.getState().selection.ids).toEqual(new Set([1, 3, 4]));
	});

	it('leaves Shift and Ctrl presses on an unselected row to the selection', async () => {
		const h = await setup();
		expect(h.press(2, { modifiers: { ctrl: true, shift: false, alt: false } })).toBe(false);
		expect(h.press(2, { modifiers: { ctrl: false, shift: true, alt: false } })).toBe(false);
		// On a selected row the same press is a drag.
		h.session.store.getState().selectEntries([3], 2);
		expect(h.press(2, { modifiers: { ctrl: true, shift: false, alt: false } })).toBe(true);
	});

	it('does not drag out of the Trash, without a queue, or while another drag runs', async () => {
		const trash = await setup({ trash: true });
		expect(trash.press()).toBe(false);
		const noQueue = await setup({ windowLabel: null });
		expect(noQueue.press()).toBe(false);
		const h = await setup();
		h.startDrag();
		expect(h.press(3)).toBe(false);
	});

	it('holds the source listing open for the drag, and lets go when it ends', async () => {
		const h = await setup();
		h.startDrag();
		expect(h.deps.retain).toHaveBeenCalledWith(1);
		expect(h.released).not.toHaveBeenCalled();
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(h.released).toHaveBeenCalledTimes(1);
	});

	it('announces the start and shows what is dragged', async () => {
		const h = await setup();
		h.startDrag();
		expect(h.announced).toContain('Dragging notes.txt');
		expect(h.pill()).toBe('Dragging notes.txt');
		expect(document.documentElement.getAttribute(FILE_DRAG_ATTRIBUTE)).toBe('idle');
	});
});

describe('the target under the pointer', () => {
	it('is none over the pane the files are in, and over nothing', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('pane', 1, 'test'));
		h.move(120);
		expect(h.target()).toBeNull();
		h.over();
		h.move(125);
		expect(h.target()).toBeNull();
	});

	it('is a folder row of the same listing, which is dropped into', async () => {
		const h = await setup();
		h.startDrag(2);
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs');
		h.over(row);
		h.move(120);
		expect(h.target()).toMatchObject({
			kind: 'folder',
			label: 'docs',
			outcome: 'copy',
			pending: true,
		});
		await settle();
		expect(h.target()?.location?.uri).toBe(DOCS.uri);
		expect(row.getAttribute('data-drop-over')).toBe('ok');
		// Leaving it clears the mark.
		h.over();
		h.move(130);
		expect(row.hasAttribute('data-drop-over')).toBe(false);
	});

	it('asks the planner about a folder row once its location has come', async () => {
		const h = await setup();
		h.startDrag(2);
		h.over(mark('folder', folderRef(h.session.model.handle, 1), 'docs'));
		h.move(120);
		await settle();
		h.clock.advance(PLAN_REST_MS);
		await settle();
		expect(h.deps.plan).toHaveBeenCalledTimes(1);
		expect(h.pill()).toBe('Move notes.txt to docs');
	});

	it('refuses a dragged folder as its own target, with a shake and the reason', async () => {
		const h = await setup();
		h.session.store.getState().selectEntries([1, 3], 0);
		h.startDrag(0);
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs');
		h.over(row);
		h.move(120);
		expect(h.target()?.blocked).toEqual({ kind: 'source' });
		expect(h.pill()).toBe('Not allowed: docs is being dragged');
		expect(row.getAttribute('data-drop-over')).toBe('blocked');
		expect(document.documentElement.getAttribute(FILE_DRAG_ATTRIBUTE)).toBe('blocked');
	});

	it('refuses a read-only folder', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('folder', '9:9', 'archive', { readOnly: true }));
		h.move(120);
		expect(h.pill()).toBe('Not allowed: archive cannot be changed');
	});

	it('refuses the folder the files are in, for a move', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('crumb', FOLDER.uri, 'test'));
		h.move(120);
		expect(h.target()?.blocked).toEqual({ kind: 'sameFolder' });
		expect(h.pill()).toBe('Not allowed: already in test');
	});

	it('uses the pane another tab shows, and is none when that is the same folder', async () => {
		const h = await setup({ panes: { 2: { location: DOCS, readOnly: false } } });
		h.startDrag();
		h.over(mark('pane', 2, 'docs'));
		h.move(120);
		expect(h.target()).toMatchObject({ kind: 'pane', location: DOCS });
		const readOnly = await setup({ panes: { 2: { location: DOCS, readOnly: true } } });
		readOnly.startDrag();
		readOnly.over(mark('pane', 2, 'docs'));
		readOnly.move(120);
		expect(readOnly.target()?.blocked).toEqual({ kind: 'readOnly' });
	});

	it('names a tab by its folder, and a sidebar place by its uri', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('tab', 2, 'music'));
		h.move(120);
		expect(h.target()).toMatchObject({ kind: 'tab', location: MUSIC });
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(130);
		expect(h.target()).toMatchObject({ kind: 'place', location: { uri: DOCS.uri } });
	});
});

describe('the default rule and the planner', () => {
	it('shows a neutral Move or copy at once, and asks the planner only after the pointer rests', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		expect(h.pill()).toBe('Move or copy notes.txt to Documents');
		expect(h.deps.plan).not.toHaveBeenCalled();
		h.clock.advance(PLAN_REST_MS - 1);
		expect(h.deps.plan).not.toHaveBeenCalled();
		h.clock.advance(1);
		expect(h.deps.plan).toHaveBeenCalledTimes(1);
		expect(h.deps.plan.mock.calls[0]![0]).toMatchObject({
			kind: { kind: 'copy' },
			sources: { kind: 'selection', spec: { kind: 'some', ids: [3] } },
			destination: { uri: DOCS.uri },
			originWindow: 'main-1',
		});
		await settle();
		expect(h.pill()).toBe('Move notes.txt to Documents');
	});

	it('copies when the planner says the target is on another volume', async () => {
		const h = await setup();
		h.state.sameVolume = false;
		h.startDrag();
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		h.clock.advance(PLAN_REST_MS);
		await settle();
		expect(h.pill()).toBe('Copy notes.txt to Documents');
	});

	it('asks the planner once per target for the whole drag, and not at all for one swept across', async () => {
		const h = await setup();
		h.startDrag();
		const docs = mark('place', DOCS.uri, 'Documents');
		const music = mark('place', MUSIC.uri, 'Music');
		h.over(docs);
		h.move(120);
		h.clock.advance(50);
		h.over(music);
		h.move(130);
		h.clock.advance(50);
		h.over(docs);
		h.move(140);
		h.clock.advance(PLAN_REST_MS);
		await settle();
		h.over(music);
		h.move(150);
		h.clock.advance(PLAN_REST_MS);
		await settle();
		h.over(docs);
		h.move(160);
		h.clock.advance(PLAN_REST_MS);
		expect(h.deps.plan).toHaveBeenCalledTimes(2);
	});

	it('shows what the planner refused', async () => {
		const h = await setup();
		h.state.planError = { error: { kind: 'intoItself' } };
		h.startDrag();
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		h.clock.advance(PLAN_REST_MS);
		await settle();
		expect(h.target()?.blocked).toEqual({ kind: 'refused', error: { kind: 'intoItself' } });
		expect(h.pill()).toBe('Not allowed: a folder cannot go into itself');
	});

	it('keeps the guess when the planner fails in a way that says nothing', async () => {
		const h = await setup();
		h.state.planError = new Error('offline');
		h.startDrag();
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		h.clock.advance(PLAN_REST_MS);
		await settle();
		expect(h.target()).toMatchObject({ outcome: 'copy', pending: true });
	});

	it('copies under always-copy, asks under always-ask, never waiting for the planner', async () => {
		const copy = await setup();
		copy.state.rule = 'alwaysCopy';
		copy.startDrag();
		copy.over(mark('place', DOCS.uri, 'Documents'));
		copy.move(120);
		expect(copy.pill()).toBe('Copy notes.txt to Documents');
		const ask = await setup();
		ask.state.rule = 'alwaysAsk';
		ask.startDrag();
		ask.over(mark('place', DOCS.uri, 'Documents'));
		ask.move(120);
		expect(ask.pill()).toBe('Choose what to do with notes.txt in Documents');
	});
});

describe('the modifiers', () => {
	const place = (h: Harness) => {
		h.state.sameVolume = true;
		h.startDrag();
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		h.clock.advance(PLAN_REST_MS);
	};

	it('change the pill live as the keys go down and up', async () => {
		const h = await setup();
		place(h);
		await settle();
		expect(h.pill()).toBe('Move notes.txt to Documents');
		fireEvent.keyDown(window, { key: 'Control', ctrlKey: true });
		expect(h.pill()).toBe('Copy notes.txt to Documents');
		fireEvent.keyDown(window, { key: 'Shift', ctrlKey: true, shiftKey: true });
		expect(h.pill()).toBe('Link notes.txt in Documents');
		fireEvent.keyUp(window, { key: 'Shift', ctrlKey: true });
		expect(h.pill()).toBe('Copy notes.txt to Documents');
		fireEvent.keyDown(window, { key: 'Alt', altKey: true });
		expect(h.pill()).toBe('Choose what to do with notes.txt in Documents');
		fireEvent.keyUp(window, { key: 'Alt' });
		expect(h.pill()).toBe('Move notes.txt to Documents');
	});

	it('are read at the drop, not at the start', async () => {
		const h = await setup();
		place(h);
		await settle();
		h.up(120, 100, { ctrlKey: true });
		await settle();
		expect(h.deps.transfer).toHaveBeenCalledWith(
			'copy',
			h.session,
			expect.objectContaining({ uri: DOCS.uri }),
		);
		const other = await setup();
		place(other);
		await settle();
		other.up(120, 100);
		await settle();
		expect(other.deps.transfer).toHaveBeenCalledWith('move', other.session, expect.anything());
	});

	it('read the keys held when the drag began', async () => {
		const h = await setup();
		h.session.store.getState().selectEntries([3], 2);
		h.startDrag(2, { modifiers: { ctrl: true, shift: false, alt: false } });
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		// The press carried Ctrl; the next pointer event, with none held, says otherwise.
		expect(h.pill()).toBe('Move or copy notes.txt to Documents');
	});

	it('links only where the engine can: Ctrl+Shift on a remote source copies', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('place', 'sftp://host/x', 'host'));
		h.move(120, 100, { ctrlKey: true, shiftKey: true });
		expect(h.target()?.outcome).toBe('copy');
	});
});

describe('the drop', () => {
	it('submits nothing without a target, and says so', async () => {
		const h = await setup();
		h.startDrag();
		h.up(120);
		await settle();
		expect(h.deps.transfer).not.toHaveBeenCalled();
		expect(h.announced).toContain('Nothing was dropped');
		expect(h.released).toHaveBeenCalledTimes(1);
	});

	it('says why a refusing target refused, and submits nothing', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('folder', '9:9', 'archive', { readOnly: true }));
		h.move(120);
		h.up(120);
		await settle();
		expect(h.say).toEqual(['Archive cannot be changed']);
		expect(h.deps.transfer).not.toHaveBeenCalled();
		expect(h.released).toHaveBeenCalledTimes(1);
	});

	it('drops into a folder row once its location has come', async () => {
		const h = await setup();
		h.startDrag(2);
		h.over(mark('folder', folderRef(h.session.model.handle, 1), 'docs'));
		h.move(120);
		h.up(120, 100, { shiftKey: true });
		await settle();
		expect(h.deps.transfer).toHaveBeenCalledWith(
			'move',
			h.session,
			expect.objectContaining({ uri: DOCS.uri }),
		);
		// The listing stays held until the job it started has ended.
		expect(h.released).toHaveBeenCalledTimes(1);
	});

	it('moves to the Trash on the Trash item, through the command that has a key', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('trash', 'trash:///', 'Trash'));
		h.move(120);
		expect(h.pill()).toBe('Move notes.txt to the Trash');
		h.up(120);
		await settle();
		expect(h.deps.moveToTrash).toHaveBeenCalledWith(h.session);
		expect(h.deps.transfer).not.toHaveBeenCalled();
	});

	it('refuses the Trash from a read-only folder', async () => {
		const h = await setup({ readOnly: true });
		h.startDrag();
		h.over(mark('trash', 'trash:///', 'Trash'));
		h.move(120);
		h.up(120);
		await settle();
		expect(h.deps.moveToTrash).not.toHaveBeenCalled();
		expect(h.say[0]).toContain('cannot be trashed from here');
	});

	it('opens tabs on + and on a chip, and a split pair with Alt', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('plus', 'new', 'New tab'));
		h.move(120);
		h.up(120);
		await settle();
		expect(h.opened[0]).toMatchObject({ target: 'plus', group: null, split: false });
		const split = await setup();
		split.startDrag();
		split.over(mark('plus', 'new', 'New tab'));
		split.move(120);
		split.up(120, 100, { altKey: true });
		await settle();
		expect(split.opened[0]).toMatchObject({ target: 'plus', split: true });
		const chip = await setup();
		chip.startDrag();
		chip.over(mark('chip', 5, 'Work'));
		chip.move(120);
		chip.up(120);
		await settle();
		expect(chip.opened[0]).toMatchObject({ target: 'chip', group: 5 });
	});

	it('never submits a job for + and a chip', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('plus', 'new', 'New tab'));
		h.move(120);
		h.up(120);
		await settle();
		expect(h.deps.transfer).not.toHaveBeenCalled();
		expect(h.deps.moveToTrash).not.toHaveBeenCalled();
	});
});

describe('the picker', () => {
	it('opens at the release point on a right-button drag, with Move and Link on offer', async () => {
		const h = await setup();
		h.startDrag(2, { button: 2 });
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		expect(h.pill()).toBe('Choose what to do with notes.txt in Documents');
		h.up(130, 140);
		await settle();
		expect(h.pickers).toHaveLength(1);
		expect(h.pickers[0]).toMatchObject({
			position: { x: 130, y: 140 },
			verbs: ['copy', 'move', 'link'],
			what: 'notes.txt',
		});
		expect(h.announced).toContain('Choose what to do with notes.txt in Documents');
		// The listing is held until the picker is answered.
		expect(h.released).not.toHaveBeenCalled();
		h.pickers[0]!.choose('link');
		await settle();
		expect(h.deps.transfer).toHaveBeenCalledWith(
			'link',
			h.session,
			expect.objectContaining({ uri: DOCS.uri }),
		);
		expect(h.released).toHaveBeenCalledTimes(1);
	});

	it('opens on Alt held at release, and cancelling submits nothing', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		h.up(120, 100, { altKey: true });
		await settle();
		h.pickers[0]!.cancel();
		expect(h.deps.transfer).not.toHaveBeenCalled();
		expect(h.announced).toContain('Drop cancelled');
		expect(h.released).toHaveBeenCalledTimes(1);
	});

	it('leaves out Move for a read-only source and for the folder the files are in, and Link across kinds of location', async () => {
		const ro = await setup({ readOnly: true });
		ro.startDrag(2, { button: 2 });
		ro.over(mark('place', DOCS.uri, 'Documents'));
		ro.move(120);
		ro.up(120);
		await settle();
		expect(ro.pickers[0]!.verbs).toEqual(['copy', 'link']);
		const here = await setup();
		here.startDrag(2, { button: 2 });
		here.over(mark('crumb', FOLDER.uri, 'test'));
		here.move(120);
		here.up(120);
		await settle();
		expect(here.pickers[0]!.verbs).toEqual(['copy', 'link']);
		const remote = await setup();
		remote.startDrag(2, { button: 2 });
		remote.over(mark('place', 'sftp://host/x', 'host'));
		remote.move(120);
		remote.up(120);
		await settle();
		expect(remote.pickers[0]!.verbs).toEqual(['copy', 'move']);
	});
});

describe('cancelling', () => {
	it('Esc cancels everything: no job, the mark cleared, the live region told', async () => {
		const h = await setup();
		h.startDrag();
		const row = mark('place', DOCS.uri, 'Documents');
		h.over(row);
		h.move(120);
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(h.drag.session.store.getState().phase).not.toBe('dragging');
		expect(h.announced).toContain('Drag cancelled');
		expect(row.hasAttribute('data-drop-over')).toBe(false);
		expect(h.deps.transfer).not.toHaveBeenCalled();
		expect(document.documentElement.hasAttribute(FILE_DRAG_ATTRIBUTE)).toBe(false);
		// What follows is not a click.
		expect(h.drag.session.consumeClick()).toBe(true);
	});

	it('clears the plan and spring timers', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		fireEvent.keyDown(window, { key: 'Escape' });
		h.clock.advance(5000);
		expect(h.deps.plan).not.toHaveBeenCalled();
		expect(h.deps.navigate).not.toHaveBeenCalled();
	});

	it('a pointer cancel ends it the same way', async () => {
		const h = await setup();
		h.startDrag();
		fireEvent.pointerCancel(window, { pointerId: 1 });
		expect(h.drag.session.store.getState().phase).not.toBe('dragging');
		expect(h.announced).toContain('Drag cancelled');
		expect(h.released).toHaveBeenCalledTimes(1);
	});
});

describe('spring-loading', () => {
	it('opens a folder row after the delay, in the pane it is in, and marks the ring while it arms', async () => {
		const h = await setup();
		h.startDrag();
		const pane = mark('pane', 1, 'test');
		pane.setAttribute('data-pane', '1');
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs', {}, pane);
		h.over(row, pane);
		h.move(120);
		expect(row.hasAttribute('data-drop-spring')).toBe(true);
		expect(row.style.getPropertyValue('--wp-drop-spring-ms')).toBe('600ms');
		h.clock.advance(599);
		expect(h.deps.navigate).not.toHaveBeenCalled();
		h.clock.advance(1);
		await settle();
		expect(h.deps.navigate).toHaveBeenCalledWith(1, expect.objectContaining({ uri: DOCS.uri }));
		expect(h.announced).toContain('Opened docs');
	});

	it('waits the delay the setting gives', async () => {
		const h = await setup();
		h.deps.springMs.mockReturnValue(200);
		h.startDrag();
		const pane = mark('pane', 1, 'test');
		pane.setAttribute('data-pane', '1');
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs', {}, pane);
		h.over(row, pane);
		h.move(120);
		h.clock.advance(200);
		await settle();
		expect(h.deps.navigate).toHaveBeenCalledTimes(1);
	});

	it('does not spring a folder the pointer left before the delay', async () => {
		const h = await setup();
		h.startDrag();
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs');
		h.over(row);
		h.move(120);
		h.clock.advance(300);
		h.over();
		h.move(125);
		h.clock.advance(1000);
		expect(h.deps.navigate).not.toHaveBeenCalled();
		expect(row.hasAttribute('data-drop-spring')).toBe(false);
	});

	it('springs back on Esc, and does not spring a dragged folder or a read-only one', async () => {
		const h = await setup();
		h.startDrag();
		const pane = mark('pane', 1, 'test');
		pane.setAttribute('data-pane', '1');
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs', {}, pane);
		h.over(row, pane);
		h.move(120);
		h.clock.advance(600);
		await settle();
		fireEvent.keyDown(window, { key: 'Escape' });
		await settle();
		expect(h.deps.back).toHaveBeenCalledWith(1);
		expect(h.announced).toContain('Returned to where the drag began');
		const readOnly = await setup();
		readOnly.startDrag();
		readOnly.over(mark('folder', '9:9', 'x', { readOnly: true }));
		readOnly.move(120);
		readOnly.clock.advance(5000);
		expect(readOnly.deps.navigate).not.toHaveBeenCalled();
	});

	it('springs back when the pointer goes to another pane, and keeps it when it stays', async () => {
		const h = await setup();
		h.startDrag();
		const pane = mark('pane', 1, 'test');
		pane.setAttribute('data-pane', '1');
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs', {}, pane);
		h.over(row, pane);
		h.move(120);
		h.clock.advance(600);
		await settle();
		// Still in the pane that was sprung: it stays open.
		h.over(pane);
		h.move(300);
		expect(h.deps.back).not.toHaveBeenCalled();
		// Another pane: it springs back.
		const other = mark('pane', 2, 'music');
		other.setAttribute('data-pane', '2');
		h.over(other);
		h.move(400);
		await settle();
		expect(h.deps.back).toHaveBeenCalledWith(1);
	});

	it('keeps the folder a drop landed in, and springs back when the drop was elsewhere', async () => {
		const kept = await setup();
		kept.startDrag();
		const pane = mark('pane', 1, 'test');
		pane.setAttribute('data-pane', '1');
		const row = mark('folder', folderRef(kept.session.model.handle, 1), 'docs', {}, pane);
		kept.over(row, pane);
		kept.move(120);
		kept.clock.advance(600);
		await settle();
		// The pane now shows docs: dropping on its file area drops into docs.
		kept.deps.pane.mockImplementation(() => ({ location: DOCS, readOnly: false }));
		kept.over(pane);
		kept.move(300);
		kept.clock.advance(PLAN_REST_MS);
		await settle();
		kept.up(300);
		await settle();
		expect(kept.deps.back).not.toHaveBeenCalled();
		expect(kept.deps.transfer).toHaveBeenCalledWith(
			'move',
			kept.session,
			expect.objectContaining({ uri: DOCS.uri }),
		);
		const away = await setup();
		away.startDrag();
		const pane2 = mark('pane', 1, 'test');
		pane2.setAttribute('data-pane', '1');
		const row2 = mark('folder', folderRef(away.session.model.handle, 1), 'docs', {}, pane2);
		away.over(row2, pane2);
		away.move(120);
		away.clock.advance(600);
		await settle();
		away.over(mark('place', MUSIC.uri, 'Music'));
		away.move(300);
		away.up(300);
		await settle();
		expect(away.deps.back).toHaveBeenCalledWith(1);
	});

	it('opens a sidebar place in the active pane, and a tab by activating it, and undoes each', async () => {
		const h = await setup();
		h.startDrag();
		h.over(mark('place', DOCS.uri, 'Documents'));
		h.move(120);
		h.clock.advance(600);
		await settle();
		expect(h.deps.navigate).toHaveBeenCalledWith(1, expect.objectContaining({ uri: DOCS.uri }));
		const tabs = await setup();
		tabs.startDrag();
		const strip = document.createElement('div');
		strip.setAttribute('data-drop-strip', '');
		document.body.append(strip);
		tabs.over(mark('tab', 2, 'music', {}, strip));
		tabs.move(120);
		tabs.clock.advance(600);
		await settle();
		expect(tabs.deps.activate).toHaveBeenCalledWith(2);
		expect(tabs.announced).toContain('Showing music');
		fireEvent.keyDown(window, { key: 'Escape' });
		await settle();
		// The tab that was active comes back.
		expect(tabs.deps.activate).toHaveBeenLastCalledWith(1);
	});

	it('reverts a spring that was still opening when the drag ended, and leaves nothing for the next drag', async () => {
		const h = await setup();
		let finish: () => void = () => {};
		h.deps.navigate.mockImplementation(
			() =>
				new Promise<void>((resolve) => {
					finish = resolve;
				}),
		);
		h.startDrag();
		const pane = mark('pane', 1, 'test');
		pane.setAttribute('data-pane', '1');
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs', {}, pane);
		h.over(row, pane);
		h.move(120);
		h.clock.advance(600);
		await settle();
		expect(h.deps.navigate).toHaveBeenCalledTimes(1);
		// Released (Esc) before the navigation settled.
		fireEvent.keyDown(window, { key: 'Escape' });
		await settle();
		finish();
		await settle();
		// The pane that switched is switched back at once, and no stale entry waits for the next drag.
		expect(h.deps.back).toHaveBeenCalledTimes(1);
		expect(h.deps.back).toHaveBeenCalledWith(1);
		h.startDrag();
		h.over(mark('place', MUSIC.uri, 'Music'));
		h.move(130);
		h.up(130);
		await settle();
		expect(h.deps.back).toHaveBeenCalledTimes(1);
	});

	it('does not open a second spring until the pointer has moved', async () => {
		const h = await setup();
		h.startDrag();
		const pane = mark('pane', 1, 'test');
		pane.setAttribute('data-pane', '1');
		const row = mark('folder', folderRef(h.session.model.handle, 1), 'docs', {}, pane);
		h.over(row, pane);
		h.move(120);
		h.clock.advance(600);
		await settle();
		// A folder under the pointer in the new view: nothing opens while it rests.
		h.clock.advance(5000);
		await settle();
		expect(h.deps.navigate).toHaveBeenCalledTimes(1);
		// Moving a little is not enough; moving past the jitter is.
		h.move(120 + SPRING_JITTER_PX + 4);
		h.clock.advance(600);
		await settle();
		expect(h.deps.navigate).toHaveBeenCalledTimes(2);
	});

	it('never springs the + button, a chip, the Trash or a breadcrumb', async () => {
		const h = await setup();
		h.startDrag();
		for (const [kind, ref, label] of [
			['plus', 'new', 'New tab'],
			['chip', 3, 'Work'],
			['trash', 'trash:///', 'Trash'],
			['crumb', DOCS.uri, 'docs'],
		] as const) {
			h.over(mark(kind, ref, label));
			h.move(120 + Math.random());
			h.clock.advance(5000);
		}
		await settle();
		expect(h.deps.navigate).not.toHaveBeenCalled();
		expect(h.deps.activate).not.toHaveBeenCalled();
	});
});

describe('scrolling at the edge', () => {
	it('scrolls the list while the pointer is near its edge, and stops when it leaves', async () => {
		const h = await setup();
		h.startDrag();
		const scroller = document.createElement('div');
		scroller.setAttribute(SCROLL_ATTRIBUTE, '');
		scroller.getBoundingClientRect = () =>
			({ top: 0, bottom: 400, left: 0, right: 300, width: 300, height: 400 }) as DOMRect;
		const pane = mark('pane', 2, 'music');
		pane.append(scroller);
		h.over(scroller, pane);
		h.move(120, 395);
		expect(scroller.scrollTop).toBe(0);
		h.clock.advance(16);
		const first = scroller.scrollTop;
		expect(first).toBeGreaterThan(0);
		h.clock.advance(32);
		expect(scroller.scrollTop).toBeGreaterThan(first);
		h.move(120, 200);
		const rest = scroller.scrollTop;
		h.clock.advance(100);
		expect(scroller.scrollTop).toBe(rest);
	});

	it('scrolls up near the top edge', async () => {
		const h = await setup();
		h.startDrag();
		const scroller = document.createElement('div');
		scroller.setAttribute(SCROLL_ATTRIBUTE, '');
		scroller.scrollTop = 500;
		scroller.getBoundingClientRect = () =>
			({ top: 0, bottom: 400, left: 0, right: 300, width: 300, height: 400 }) as DOMRect;
		const pane = mark('pane', 2, 'music');
		pane.append(scroller);
		h.over(scroller, pane);
		h.move(120, 2);
		h.clock.advance(16);
		expect(scroller.scrollTop).toBeLessThan(500);
	});

	it('stops when the drag ends', async () => {
		const h = await setup();
		h.startDrag();
		const scroller = document.createElement('div');
		scroller.setAttribute(SCROLL_ATTRIBUTE, '');
		scroller.getBoundingClientRect = () =>
			({ top: 0, bottom: 400, left: 0, right: 300, width: 300, height: 400 }) as DOMRect;
		const pane = mark('pane', 2, 'music');
		pane.append(scroller);
		h.over(scroller, pane);
		h.move(120, 395);
		fireEvent.keyDown(window, { key: 'Escape' });
		const stopped = scroller.scrollTop;
		h.clock.advance(500);
		expect(scroller.scrollTop).toBe(stopped);
	});
});

describe('a right-button press', () => {
	it('holds the context menu back until the release, and a drag drops it', async () => {
		const h = await setup();
		const open = vi.fn();
		expect(h.drag.deferMenu(open)).toBe(false);
		h.press(2, { button: 2 });
		expect(h.drag.deferMenu(open)).toBe(true);
		expect(open).not.toHaveBeenCalled();
		h.up(100);
		expect(open).toHaveBeenCalledTimes(1);

		const dragged = await setup();
		const never = vi.fn();
		dragged.press(2, { button: 2 });
		dragged.drag.deferMenu(never);
		dragged.move(140);
		dragged.up(140);
		expect(never).not.toHaveBeenCalled();
	});

	it('does not hold the menu for a left press, or one that has become a drag', async () => {
		const h = await setup();
		h.press(2);
		expect(h.drag.deferMenu(vi.fn())).toBe(false);
		h.up(100);
		const dragging = await setup();
		dragging.press(2, { button: 2 });
		dragging.move(140);
		expect(dragging.drag.deferMenu(vi.fn())).toBe(false);
	});
});

describe('disposal', () => {
	it('ends a drag in progress', async () => {
		const h = await setup();
		h.startDrag();
		h.drag.dispose();
		expect(h.drag.session.store.getState().phase).not.toBe('dragging');
		expect(h.released).toHaveBeenCalledTimes(1);
	});
});
