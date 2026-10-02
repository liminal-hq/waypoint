// Verifies the Shelf as a drop target and a drag source in the drag's pure rules: the verdicts, the pill and the parity tables
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import type { ListingSession } from '../browse/useListingSession';
import { fileLocation } from '../services/fakeVfsClient';
import { NO_MODIFIERS } from './dropAction';
import type { DropKind, DropSpot } from './dropTargets';
import {
	blockedText,
	evaluateTarget,
	NON_POINTER_PATHS,
	pillFor,
	TARGET_PATHS,
	type EvaluateInput,
	type FileDragSource,
	type LocationsDragSource,
	type SelectionDragSource,
} from './fileDragModel';

const HOME = fileLocation('/home/test');
const DOCS = fileLocation('/home/test/docs');

const listing = (over: Partial<SelectionDragSource> = {}): SelectionDragSource => ({
	session: {} as ListingSession,
	tab: 1,
	handle: 7,
	spec: { kind: 'some', ids: [1, 2, 3] },
	count: 3,
	name: null,
	groups: ['document', 'document', 'document'],
	folder: HOME,
	readOnly: false,
	rightButton: false,
	...over,
});

const shelfSource = (over: Partial<LocationsDragSource> = {}): LocationsDragSource => ({
	kind: 'locations',
	locations: [fileLocation('/home/test/a.txt'), fileLocation('/home/test/b.txt')],
	tab: null,
	count: 2,
	name: null,
	groups: ['other', 'other'],
	folder: HOME,
	readOnly: false,
	rightButton: false,
	...over,
});

const spot = (kind: DropKind, over: Partial<DropSpot> = {}): DropSpot => ({
	kind,
	ref: 'x',
	label: 'Shelf',
	readOnly: false,
	unavailable: false,
	element: document.createElement('div'),
	pane: null,
	inStrip: false,
	scroller: null,
	...over,
});

const evaluate = (source: FileDragSource, over: Partial<EvaluateInput> = {}) =>
	evaluateTarget({
		source,
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

describe('the Shelf as a target', () => {
	it('takes a listing’s selection as references, with no plan, no volume and no refusal', () => {
		const target = evaluate(listing(), { spot: spot('shelf'), location: null, plan: null });
		expect(target).toMatchObject({
			kind: 'shelf',
			outcome: 'shelf',
			blocked: null,
			pending: false,
		});
	});

	it('is not refused by a read-only source, or by the modifiers', () => {
		const held = { ctrl: true, shift: true, alt: true };
		for (const source of [listing({ readOnly: true }), listing({ rightButton: true })]) {
			expect(
				evaluate(source, { spot: spot('shelf'), location: null, modifiers: held }).outcome,
			).toBe('shelf');
		}
	});

	it('says what it will do: add the items, by name or count', () => {
		const many = pillFor(listing(), evaluate(listing(), { spot: spot('shelf'), location: null }));
		expect(many).toMatchObject({ text: 'Add 3 items to the Shelf', kind: 'shelf' });
		expect(many.announce).toBe('Over Shelf: will add to the Shelf');
		const one = listing({ count: 1, name: 'report.pdf' });
		expect(pillFor(one, evaluate(one, { spot: spot('shelf'), location: null })).text).toBe(
			'Add report.pdf to the Shelf',
		);
	});

	it('refuses items that came from the Shelf, which are already on it', () => {
		const target = evaluate(shelfSource(), { spot: spot('shelf'), location: null });
		expect(target.blocked).toEqual({ kind: 'onShelf' });
		expect(blockedText(target.blocked!, target)).toBe('these items are already on the Shelf');
	});
});

describe('the Shelf as a source', () => {
	it('is dropped into a folder by the usual rule: a move on one volume, a copy across', () => {
		expect(evaluate(shelfSource()).outcome).toBe('move');
		expect(evaluate(shelfSource(), { plan: { volume: 'different', error: null } }).outcome).toBe(
			'copy',
		);
		expect(evaluate(shelfSource(), { plan: null })).toMatchObject({
			outcome: 'copy',
			pending: true,
		});
	});

	it('follows the modifiers and the rule like any other drag', () => {
		const ctrl = { ctrl: true, shift: false, alt: false };
		expect(evaluate(shelfSource(), { modifiers: ctrl }).outcome).toBe('copy');
		expect(evaluate(shelfSource(), { rule: 'alwaysAsk' }).outcome).toBe('ask');
		expect(evaluate(shelfSource({ rightButton: true })).outcome).toBe('ask');
	});

	it('cannot move into the folder every item is in, but can copy there (a duplicate)', () => {
		const here = { location: HOME, plan: null };
		expect(evaluate(shelfSource(), here).blocked).toEqual({ kind: 'sameFolder' });
		expect(
			evaluate(shelfSource(), { ...here, modifiers: { ctrl: true, shift: false, alt: false } })
				.outcome,
		).toBe('copy');
	});

	it('can move into any folder when the items come from several, because none is the one they are in', () => {
		const mixed = shelfSource({ folder: null });
		expect(evaluate(mixed, { location: HOME, plan: null }).outcome).toBe('copy');
		expect(evaluate(mixed, { location: HOME, plan: { volume: 'same', error: null } }).outcome).toBe(
			'move',
		);
	});

	it('has nothing to trash or open: the Trash, the + button and a group chip refuse it', () => {
		for (const kind of ['trash', 'plus', 'chip'] as const) {
			const target = evaluate(shelfSource(), { spot: spot(kind), location: null });
			expect(target.blocked, kind).toEqual({ kind: 'shelfSource' });
			expect(blockedText(target.blocked!, target)).toBe('items from the Shelf go into folders');
		}
	});

	it('refuses a folder that cannot be written to, as any drag does', () => {
		expect(evaluate(shelfSource(), { readOnly: true }).blocked).toEqual({ kind: 'readOnly' });
	});
});

describe('the parity tables', () => {
	it('reach the Shelf without a pointer through Add to Shelf', () => {
		expect(NON_POINTER_PATHS.shelf).toEqual({ kind: 'menu', item: 'addToShelf' });
		expect(TARGET_PATHS.shelf).toBe('addToShelf');
	});
});
