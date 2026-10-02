// Verifies the default drop action: the rule, the volume, the modifiers read at release and the right button
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	chooseDropAction,
	modifiersOf,
	NO_MODIFIERS,
	pickerVerbs,
	type DropActionInput,
	type DropModifiers,
	type DropVerb,
	type VolumeRelation,
} from './dropAction';

const input = (over: Partial<DropActionInput> = {}): DropActionInput => ({
	rule: 'byVolume',
	volume: 'same',
	modifiers: NO_MODIFIERS,
	rightButton: false,
	canMove: true,
	canLink: true,
	...over,
});

const mods = (...held: Array<'ctrl' | 'shift' | 'alt'>): DropModifiers => ({
	ctrl: held.includes('ctrl'),
	shift: held.includes('shift'),
	alt: held.includes('alt'),
});

describe('the rule with no modifier', () => {
	const cases: Array<[DropActionInput['rule'], VolumeRelation, DropVerb, boolean]> = [
		['byVolume', 'same', 'move', false],
		['byVolume', 'different', 'copy', false],
		['byVolume', 'unknown', 'copy', true],
		['alwaysCopy', 'same', 'copy', false],
		['alwaysCopy', 'different', 'copy', false],
		['alwaysCopy', 'unknown', 'copy', false],
		['alwaysAsk', 'same', 'ask', false],
		['alwaysAsk', 'different', 'ask', false],
		['alwaysAsk', 'unknown', 'ask', false],
	];
	it.each(cases)('%s on %s volume is %s (pending %s)', (rule, volume, verb, pending) => {
		expect(chooseDropAction(input({ rule, volume }))).toEqual({ verb, pending, forced: false });
	});

	it('copies instead of moving out of a folder that cannot be written to', () => {
		expect(chooseDropAction(input({ canMove: false })).verb).toBe('copy');
	});
});

describe('the modifiers, whatever the rule says', () => {
	const rules: Array<DropActionInput['rule']> = ['byVolume', 'alwaysCopy', 'alwaysAsk'];
	const volumes: VolumeRelation[] = ['same', 'different', 'unknown'];
	for (const rule of rules) {
		for (const volume of volumes) {
			it(`Ctrl copies, Shift moves and Ctrl+Shift links under ${rule} on ${volume}`, () => {
				const choose = (held: DropModifiers) =>
					chooseDropAction(input({ rule, volume, modifiers: held }));
				expect(choose(mods('ctrl'))).toEqual({ verb: 'copy', pending: false, forced: true });
				expect(choose(mods('shift'))).toEqual({ verb: 'move', pending: false, forced: true });
				expect(choose(mods('ctrl', 'shift'))).toEqual({
					verb: 'link',
					pending: false,
					forced: true,
				});
			});
		}
	}

	it('opens the picker with Alt, with or without the other keys', () => {
		for (const held of [
			mods('alt'),
			mods('alt', 'ctrl'),
			mods('alt', 'shift'),
			mods('alt', 'ctrl', 'shift'),
		]) {
			expect(chooseDropAction(input({ modifiers: held })).verb).toBe('ask');
		}
	});

	it('opens the picker for a right-button drag, with or without keys', () => {
		expect(chooseDropAction(input({ rightButton: true })).verb).toBe('ask');
		expect(chooseDropAction(input({ rightButton: true, modifiers: mods('ctrl') })).verb).toBe(
			'ask',
		);
		expect(chooseDropAction(input({ rightButton: true, rule: 'alwaysCopy' })).forced).toBe(true);
	});

	it('copies for Ctrl+Shift where the engine cannot link, and for Shift where nothing can be moved', () => {
		expect(chooseDropAction(input({ modifiers: mods('ctrl', 'shift'), canLink: false })).verb).toBe(
			'copy',
		);
		expect(chooseDropAction(input({ modifiers: mods('shift'), canMove: false })).verb).toBe('copy');
	});

	it('is read from the modifiers it is given, so a key pressed at release changes the answer', () => {
		const before = chooseDropAction(input({ volume: 'same' }));
		const after = chooseDropAction(input({ volume: 'same', modifiers: mods('ctrl') }));
		expect([before.verb, after.verb]).toEqual(['move', 'copy']);
	});
});

describe('the picker', () => {
	it('offers Copy, then Move and Link where they apply', () => {
		expect(pickerVerbs({ canMove: true, canLink: true })).toEqual(['copy', 'move', 'link']);
		expect(pickerVerbs({ canMove: false, canLink: true })).toEqual(['copy', 'link']);
		expect(pickerVerbs({ canMove: true, canLink: false })).toEqual(['copy', 'move']);
		expect(pickerVerbs({ canMove: false, canLink: false })).toEqual(['copy']);
	});
});

describe('modifiersOf', () => {
	it('reads an event', () => {
		expect(modifiersOf({ ctrlKey: true, shiftKey: false, altKey: true })).toEqual(
			mods('ctrl', 'alt'),
		);
	});
});

describe('files from another application', () => {
	it('are copied under the by-volume rule even on one volume, and moved only by a modifier or the picker', () => {
		const foreign = (over: Partial<DropActionInput> = {}) =>
			chooseDropAction(input({ foreign: true, ...over }));
		expect(foreign()).toEqual({ verb: 'copy', pending: false, forced: false });
		expect(foreign({ volume: 'unknown' })).toEqual({ verb: 'copy', pending: false, forced: false });
		expect(foreign({ modifiers: mods('shift') }).verb).toBe('move');
		expect(foreign({ rightButton: true }).verb).toBe('ask');
		// The other rules are the person's choice and stay as they are.
		expect(foreign({ rule: 'alwaysAsk' }).verb).toBe('ask');
	});
});
