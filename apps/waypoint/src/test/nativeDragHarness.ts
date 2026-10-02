// A file drag over fake dependencies, for the tests of drags the system runs: files dragged in and drags that leave the window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { fireEvent } from '@testing-library/react';
import { expect, vi } from 'vitest';
import { openListingModel } from '../browse/listingModel';
import { createListingSession, type ListingSession } from '../browse/useListingSession';
import type { DragClock } from '../dnd/dragSession';
import { dropAttributes } from '../dnd/dropTargets';
import {
	createFileDrag,
	type FileDrag,
	type FileDragDeps,
	type NativeFeed,
	type OpenFoldersRequest,
	type OutboundDeps,
	type PickerRequest,
} from '../dnd/fileDrag';
import type { FileDropTarget } from '../dnd/fileDragModel';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import type { OutboundRequest, OutboundStarted } from '../services/nativeDndClient';
import type { Location, PlanPreview } from '../services/opsClient';
import { FOLDER } from './browseHarness';

/** A clock the test winds by hand, so holds and ticks need no waiting. */
export class ManualClock implements DragClock {
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

export const DOCS = fileLocation('/home/test/docs');
export const MUSIC = fileLocation('/home/test/music');
export const ENTRIES = [
	makeEntry(1, 'docs', { kind: 'directory' }),
	makeEntry(2, 'music', { kind: 'directory' }),
	makeEntry(3, 'notes.txt'),
	makeEntry(4, 'photo.jpg'),
];

/** Files another application is dragging: a name with a space and one that is not UTF-8. */
export const OUTSIDE = [
	{ display: '/srv/share/with space.txt', uri: 'file:///srv/share/with%20space.txt' },
	{ display: '/srv/share/bad��.txt', uri: 'file:///srv/share/bad%FF%FE.txt' },
];

/** The one file the first row drag carries out of the window, as it comes back. */
export const OWN = [{ display: '/home/test/notes.txt', uri: 'file:///home/test/notes.txt' }];

export const settle = async () => {
	for (let i = 0; i < 8; i++) await Promise.resolve();
};

const planOf = (sameVolume: boolean): PlanPreview => ({
	kind: { kind: 'copy' },
	sources: { count: 1, first: 'a', bytes: 1 } as unknown as PlanPreview['sources'],
	items: 1,
	bytes: 1,
	sameVolume,
	conflicts: [],
	notes: [],
});

export interface NativeHarness {
	drag: FileDrag;
	session: ListingSession;
	clock: ManualClock;
	/** The hit test's stack, topmost first. */
	stack: Element[];
	say: string[];
	announced: string[];
	pickers: PickerRequest[];
	opened: OpenFoldersRequest[];
	plans: unknown[];
	transfers: Array<{ kind: string; items: Location[]; destination: Location }>;
	moved: Array<{ kind: string; destination: Location }>;
	state: {
		sameVolume: boolean;
		rule: 'byVolume' | 'alwaysCopy' | 'alwaysAsk';
		/** Where the pointer is: outside the window when `true`. */
		outside: (point: { x: number; y: number }) => boolean;
		outboundAvailable: boolean;
		resolve: () => Promise<Location[]>;
		start: (request: OutboundRequest) => Promise<OutboundStarted>;
	};
	started: OutboundRequest[];
	navigate: ReturnType<typeof vi.fn>;
	activate: ReturnType<typeof vi.fn>;
	/** Begins a drag of files the system is dragging in. */
	enter(
		files?: Location[],
		point?: { x: number; y: number },
		keys?: { ctrl?: boolean; shift?: boolean; alt?: boolean },
	): NativeFeed;
	target(): FileDropTarget | null;
	pill(): string | undefined;
	phase(): string;
	over(...elements: Element[]): void;
	/** Presses a row and drags 10 px: an in-page drag has begun. */
	startDrag(position?: number, over?: Partial<Parameters<FileDrag['press']>[0]>): void;
	move(x: number, y?: number): void;
	up(x?: number, y?: number): void;
}

const NO_KEYS = { ctrl: false, shift: false, alt: false };

const created: FileDrag[] = [];

/** Ends every drag the harness made, so none keeps listening to the window for the next test. */
export function disposeHarnesses(): void {
	for (const drag of created.splice(0)) drag.dispose();
}

export async function nativeHarness(
	options: {
		readOnly?: boolean;
		outbound?: boolean;
		windowLabel?: string | null;
		entries?: Entry[];
		/** Where the listing is; the test folder when omitted. */
		folder?: Location;
	} = {},
): Promise<NativeHarness> {
	const vfs = new FakeVfsClient();
	const entries = options.entries ?? ENTRIES;
	const folder = options.folder ?? FOLDER;
	vfs.setFolder(folder, entries);
	if (options.readOnly) vfs.setReadOnly(folder);
	const session = createListingSession(await openListingModel(vfs, folder));
	const clock = new ManualClock();
	const say: string[] = [];
	const announced: string[] = [];
	const pickers: PickerRequest[] = [];
	const opened: OpenFoldersRequest[] = [];
	const plans: unknown[] = [];
	const transfers: NativeHarness['transfers'] = [];
	const moved: NativeHarness['moved'] = [];
	const started: OutboundRequest[] = [];
	const stack: Element[] = [];
	const state: NativeHarness['state'] = {
		sameVolume: true,
		rule: 'byVolume',
		outside: (point) => point.x < 0 || point.x >= 800 || point.y < 0 || point.y >= 600,
		outboundAvailable: options.outbound ?? true,
		resolve: async () => [fileLocation('/home/test/notes.txt')],
		start: async (request) => {
			started.push(request);
			return { id: started.length, ended: null };
		},
	};
	const outbound: OutboundDeps = {
		available: () => state.outboundAvailable,
		outside: (point) => state.outside(point),
		resolve: () => state.resolve(),
		start: (request) => state.start(request),
	};
	const navigate = vi.fn(async () => {});
	const activate = vi.fn(async () => {});
	const deps: FileDragDeps = {
		rule: () => state.rule,
		springMs: () => 600,
		announce: (text) => void announced.push(text),
		say: (text) => void say.push(text),
		windowLabel: () => (options.windowLabel === undefined ? 'main-1' : options.windowLabel),
		plan: async (request) => {
			plans.push(request);
			return planOf(state.sameVolume);
		},
		entryLocation: (handle, entry) => vfs.entryLocation(handle, entry),
		pane: () => ({ location: FOLDER, readOnly: false }),
		tabLocation: (tab) => (tab === 2 ? MUSIC : tab === 1 ? FOLDER : null),
		activeTab: () => 1,
		navigate,
		back: async () => {},
		activate,
		retain: () => () => {},
		transfer: async (kind, _session, destination) => void moved.push({ kind, destination }),
		transferLocations: async (kind, items, destination) =>
			void transfers.push({ kind, items, destination }),
		moveToTrash: async () => {},
		openFolders: async (request) => void opened.push(request),
		openPicker: (request) => void pickers.push(request),
		trashAvailable: () => true,
		outbound,
		hit: { elementsFromPoint: () => stack },
		clock,
		reducedMotion: () => true,
	};
	const drag = createFileDrag(deps);
	created.push(drag);
	const element = document.createElement('div');
	document.body.append(element);
	element.setPointerCapture = vi.fn();
	element.releasePointerCapture = vi.fn();
	const harness: NativeHarness = {
		drag,
		session,
		clock,
		stack,
		say,
		announced,
		pickers,
		opened,
		plans,
		transfers,
		moved,
		state,
		started,
		navigate,
		activate,
		enter(files = OUTSIDE, point = { x: 100, y: 100 }, keys = {}) {
			const feed = drag.beginNative({ files, point, modifiers: { ...NO_KEYS, ...keys } });
			expect(feed).not.toBeNull();
			return feed!;
		},
		target: () => drag.session.store.getState().target,
		pill: () => drag.session.store.getState().pill?.text,
		phase: () => drag.session.store.getState().phase,
		over(...elements) {
			stack.splice(0, stack.length, ...elements);
		},
		startDrag(position = 2, over = {}) {
			const entry = entries[position]!;
			expect(
				drag.press({
					pointerId: 1,
					clientX: 100,
					clientY: 100,
					button: 0,
					element,
					session,
					position,
					entry,
					tab: 1,
					modifiers: NO_KEYS,
					...over,
				}),
			).toBe(true);
			harness.move(110);
			expect(harness.phase()).toBe('dragging');
		},
		move(x, y = 100) {
			fireEvent.pointerMove(window, { clientX: x, clientY: y, pointerId: 1 });
		},
		up(x = 200, y = 100) {
			fireEvent.pointerUp(window, { clientX: x, clientY: y, pointerId: 1 });
		},
	};
	return harness;
}

/** A marked element in the document. */
export function mark(
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
