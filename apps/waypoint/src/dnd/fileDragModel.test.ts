// Verifies the verdict for each target (what a release does, and why it refuses) and the words of the pill
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { ListingSession } from '../browse/useListingSession';
import { NO_MODIFIERS, type DropModifiers } from './dropAction';
import type { DropKind, DropSpot } from './dropTargets';
import {
	blockedText,
	dragText,
	evaluateTarget,
	isTrashUri,
	NON_POINTER_PATHS,
	pillFor,
	sameFileTarget,
	subjectText,
	TARGET_PATHS,
	transferOf,
	type EvaluateInput,
	type FileDragSource,
	type SelectionDragSource,
	type FileDropTarget,
	type PlanFact,
} from './fileDragModel';

const HOME = { display: '/home/test', uri: 'file:///home/test' };
const DOCS = { display: '/home/test/docs', uri: 'file:///home/test/docs' };

const source = (over: Partial<SelectionDragSource> = {}): FileDragSource => ({
	session: {} as ListingSession,
	tab: 1,
	handle: 7,
	spec: { kind: 'some', ids: [1] },
	count: 1,
	name: 'a.txt',
	groups: ['document'],
	folder: HOME,
	readOnly: false,
	rightButton: false,
	...over,
});

const spot = (kind: DropKind, over: Partial<DropSpot> = {}): DropSpot => ({
	kind,
	ref: 'x',
	label: 'Docs',
	readOnly: false,
	unavailable: false,
	element: document.createElement('div'),
	pane: null,
	inStrip: false,
	scroller: null,
	...over,
});

const evaluate = (over: Partial<EvaluateInput> = {}): FileDropTarget =>
	evaluateTarget({
		source: source(),
		spot: spot('place'),
		location: DOCS,
		readOnly: false,
		selfRow: false,
		plan: { volume: 'same', error: null },
		modifiers: NO_MODIFIERS,
		rule: 'byVolume',
		canLink: true,
		trashAvailable: true,
		...over,
	});

describe('evaluateTarget', () => {
	it('moves on one volume and copies across, by default', () => {
		expect(evaluate().outcome).toBe('move');
		expect(evaluate({ plan: { volume: 'different', error: null } }).outcome).toBe('copy');
	});

	it('guesses a copy, marked pending, until the planner has answered', () => {
		const target = evaluate({ plan: null });
		expect([target.outcome, target.pending, target.volume]).toEqual(['copy', true, 'unknown']);
	});

	it('follows the modifiers', () => {
		const held = (modifiers: DropModifiers) => evaluate({ modifiers }).outcome;
		expect(held({ ctrl: true, shift: false, alt: false })).toBe('copy');
		expect(held({ ctrl: false, shift: true, alt: false })).toBe('move');
		expect(held({ ctrl: true, shift: true, alt: false })).toBe('link');
		expect(held({ ctrl: false, shift: false, alt: true })).toBe('ask');
	});

	it('asks on a right-button drag and under the always-ask rule', () => {
		expect(evaluate({ source: source({ rightButton: true }) }).outcome).toBe('ask');
		expect(evaluate({ rule: 'alwaysAsk' }).outcome).toBe('ask');
		expect(evaluate({ rule: 'alwaysCopy' }).outcome).toBe('copy');
	});

	it('refuses the folder the files are in for a move, and copies there (a duplicate)', () => {
		const here = { location: HOME, plan: null };
		expect(evaluate(here).blocked).toEqual({ kind: 'sameFolder' });
		expect(evaluate({ ...here, modifiers: { ctrl: true, shift: false, alt: false } }).outcome).toBe(
			'copy',
		);
		// Spelled differently, it is still the same folder.
		expect(evaluate({ location: { ...HOME, uri: 'file:///home/test/' } }).blocked).toEqual({
			kind: 'sameFolder',
		});
	});

	it('refuses a dragged folder as its own target', () => {
		expect(evaluate({ selfRow: true, spot: spot('folder') }).blocked).toEqual({ kind: 'source' });
	});

	it('refuses a read-only folder, from the spot or from the pane', () => {
		expect(evaluate({ readOnly: true }).blocked).toEqual({ kind: 'readOnly' });
		expect(evaluate({ spot: spot('folder', { readOnly: true }) }).blocked).toEqual({
			kind: 'readOnly',
		});
	});

	it('refuses the Trash as a folder, and what the planner refused', () => {
		expect(evaluate({ location: { display: 'Trash', uri: 'trash:///' } }).blocked).toEqual({
			kind: 'trashView',
		});
		const plan: PlanFact = { volume: 'unknown', error: { kind: 'intoItself' } };
		expect(evaluate({ plan }).blocked).toEqual({ kind: 'refused', error: { kind: 'intoItself' } });
	});

	it('moves to the Trash on the Trash item, and refuses where that is not possible', () => {
		const trash = spot('trash');
		expect(evaluate({ spot: trash, location: null }).outcome).toBe('trash');
		expect(evaluate({ spot: trash, source: source({ readOnly: true }) }).blocked).toEqual({
			kind: 'trashSource',
		});
		expect(
			evaluate({ spot: trash, source: source({ folder: { display: 'Trash', uri: 'trash:///' } }) })
				.blocked,
		).toEqual({ kind: 'trashSource' });
		expect(evaluate({ spot: spot('trash', { unavailable: true }) }).blocked).toEqual({
			kind: 'unavailable',
		});
		expect(evaluate({ spot: trash, trashAvailable: false }).blocked).toEqual({
			kind: 'unavailable',
		});
	});

	it('opens tabs on the + button and a chip, whatever the files are', () => {
		expect(evaluate({ spot: spot('plus'), location: null }).outcome).toBe('open');
		expect(evaluate({ spot: spot('chip'), location: null, plan: null }).outcome).toBe('open');
	});

	it('never moves out of a read-only folder: the default and Shift copy, and the picker has no Move', () => {
		const readOnlySource = source({ readOnly: true });
		expect(evaluate({ source: readOnlySource }).outcome).toBe('copy');
		expect(
			evaluate({ source: readOnlySource, modifiers: { ctrl: false, shift: true, alt: false } })
				.outcome,
		).toBe('copy');
	});
});

describe('the pill', () => {
	const pill = (over: Partial<EvaluateInput> = {}, src = source(), alt = false) =>
		pillFor(src, evaluate({ source: src, ...over }), alt);

	it('says what a release does, to what, and where', () => {
		expect(pill()).toMatchObject({ text: 'Move a.txt to Docs', kind: 'move' });
		expect(pill({ plan: { volume: 'different', error: null } }).text).toBe('Copy a.txt to Docs');
		expect(pill({ modifiers: { ctrl: true, shift: true, alt: false } }).text).toBe(
			'Link a.txt in Docs',
		);
		const many = source({ count: 3, name: null });
		expect(pill({}, many).text).toBe('Move 3 items to Docs');
		expect(pill({ plan: { volume: 'different', error: null } }, many).text).toBe(
			'Copy 3 items to Docs',
		);
	});

	it('is neutral while the volume is unknown', () => {
		expect(pill({ plan: null }, source({ count: 3, name: null }))).toMatchObject({
			text: 'Move or copy 3 items to Docs',
			kind: 'pending',
			announce: 'Over Docs: will move or copy',
		});
	});

	it('says the live region sentence for the same state', () => {
		expect(pill().announce).toBe('Over Docs: will move');
		expect(pill({ modifiers: { ctrl: true, shift: false, alt: false } }).announce).toBe(
			'Over Docs: will copy',
		);
		expect(pill({ rule: 'alwaysAsk' }).announce).toBe('Over Docs: will ask what to do');
	});

	it('says why a target refuses', () => {
		expect(pill({ readOnly: true })).toMatchObject({
			text: 'Not allowed: Docs cannot be changed',
			kind: 'blocked',
			announce: 'Over Docs: not allowed, Docs cannot be changed',
		});
		expect(pill({ location: HOME, plan: null }).text).toBe('Not allowed: already in Docs');
		expect(pill({ plan: { volume: 'unknown', error: { kind: 'intoItself' } } }).text).toBe(
			'Not allowed: a folder cannot go into itself',
		);
		expect(
			pill({ plan: { volume: 'unknown', error: { kind: 'io', message: 'disk on fire' } } }).text,
		).toContain('Not allowed:');
	});

	it('says what the Trash, +, a chip and a split do', () => {
		expect(pill({ spot: spot('trash'), location: null }).text).toBe('Move a.txt to the Trash');
		expect(pill({ spot: spot('plus'), location: null }).text).toBe('Open in a new tab');
		expect(
			pill({ spot: spot('plus'), location: null }, source({ count: 3, name: null })).text,
		).toBe('Open in new tabs');
		expect(pill({ spot: spot('plus'), location: null }, source(), true).text).toBe(
			'Open in a split pair',
		);
		expect(pill({ spot: spot('chip', { label: 'Work' }), location: null }).text).toBe(
			'Open in a new tab in Work',
		);
	});

	it('says what is being dragged when nothing is under the pointer', () => {
		expect(pillFor(source(), null)).toEqual({ text: 'Dragging a.txt', kind: 'idle' });
		expect(pillFor(source({ count: 3, name: null }), null).text).toBe('Dragging 3 items');
	});
});

describe('words', () => {
	it('names one item or counts several', () => {
		expect(subjectText({ count: 1, name: 'a.txt' })).toBe('a.txt');
		expect(subjectText({ count: 1, name: null })).toBe('1 item');
		expect(subjectText({ count: 1200, name: null })).toBe('1,200 items');
		expect(dragText({ count: 1, name: 'a.txt' })).toBe('Dragging a.txt');
		expect(dragText({ count: 1, name: null })).toBe('Dragging 1 item');
	});

	it('words every reason', () => {
		const target = { label: 'Docs' };
		for (const kind of [
			'sameFolder',
			'intoItself',
			'source',
			'readOnly',
			'trashView',
			'trashSource',
			'unavailable',
		] as const) {
			expect(blockedText({ kind }, target)).not.toBe('');
		}
		expect(blockedText({ kind: 'refused', error: { kind: 'intoItself' } }, target)).toBe(
			'a folder cannot go into itself',
		);
	});

	it('knows the Trash scheme', () => {
		expect(isTrashUri('trash:///')).toBe(true);
		expect(isTrashUri('file:///trash')).toBe(false);
	});
});

describe('sameFileTarget', () => {
	it('is the same only when nothing that is drawn differs', () => {
		const a = evaluate();
		expect(sameFileTarget(a, { ...a })).toBe(true);
		expect(sameFileTarget(a, { ...a, outcome: 'copy' })).toBe(false);
		expect(sameFileTarget(a, { ...a, pending: true })).toBe(false);
		expect(sameFileTarget(a, { ...a, location: HOME })).toBe(false);
		expect(sameFileTarget(a, { ...a, blocked: { kind: 'readOnly' } })).toBe(false);
	});
});

describe('the paths without a pointer', () => {
	it('has one for every outcome and every kind of target', () => {
		expect(Object.keys(NON_POINTER_PATHS).sort()).toEqual(
			['ask', 'copy', 'link', 'move', 'open', 'shelf', 'trash'].sort(),
		);
		expect(Object.keys(TARGET_PATHS).sort()).toEqual(
			['chip', 'crumb', 'folder', 'pane', 'place', 'plus', 'shelf', 'tab', 'trash'].sort(),
		);
	});
});

describe('drops that reach a server (D151)', () => {
	const name = (login: string) => (login === 'sftp://me@nas.lan' ? 'NAS' : login);

	it('reads which way the files go from the logins the planner found', () => {
		expect(transferOf({ from: [], to: 'sftp://me@nas.lan' }, name)).toEqual({
			way: 'upload',
			server: 'NAS',
		});
		expect(transferOf({ from: ['sftp://me@nas.lan'], to: null }, name)).toEqual({
			way: 'download',
			server: 'NAS',
		});
		expect(transferOf({ from: ['dav://x'], to: 'sftp://me@nas.lan' }, name)).toEqual({
			way: 'across',
			server: 'NAS',
		});
		expect(transferOf({ from: [], to: null }, name)).toBeUndefined();
		expect(transferOf(undefined, name)).toBeUndefined();
	});

	it('says Upload, Download, or where a move goes, and reads it out', () => {
		const upload: PlanFact = {
			volume: 'different',
			error: null,
			transfer: { way: 'upload', server: 'NAS' },
		};
		const copy = evaluate({ plan: upload });
		expect(copy.outcome).toBe('copy');
		const pill = pillFor({ count: 3, name: null }, copy);
		expect(pill.text).toBe('Upload 3 items to Docs on NAS');
		expect(pill.announce).toBe('Over Docs on NAS: will upload');

		const download = evaluate({
			plan: { volume: 'different', error: null, transfer: { way: 'download', server: 'NAS' } },
		});
		expect(pillFor({ count: 1, name: 'a.txt' }, download).text).toBe(
			'Download a.txt from NAS to Docs',
		);

		const moved = evaluate({
			plan: upload,
			modifiers: { ctrl: false, shift: true, alt: false },
		});
		expect(moved.outcome).toBe('move');
		expect(pillFor({ count: 1, name: 'a.txt' }, moved).text).toBe('Move a.txt to Docs on NAS');

		const across = evaluate({
			plan: { volume: 'different', error: null, transfer: { way: 'across', server: 'NAS' } },
		});
		expect(pillFor({ count: 1, name: 'a.txt' }, across).text).toBe('Copy a.txt to Docs on NAS');
	});

	it('keeps the plain words for a drop between local folders and while the planner thinks', () => {
		expect(pillFor({ count: 1, name: 'a.txt' }, evaluate({ plan: null })).text).toBe(
			'Move or copy a.txt to Docs',
		);
		expect(
			pillFor({ count: 1, name: 'a.txt' }, evaluate({ plan: { volume: 'different', error: null } }))
				.text,
		).toBe('Copy a.txt to Docs');
	});
});
