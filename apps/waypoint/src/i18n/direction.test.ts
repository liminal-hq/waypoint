// Tests reading an element's direction and swapping the arrow keys for it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { endScrollLeft, inlineKey, isRtl, overflowSides, wheelScrollDelta } from './direction';

afterEach(() => {
	document.documentElement.style.removeProperty('direction');
});

describe('isRtl', () => {
	it('follows the direction set on the root down to an element', () => {
		const child = document.body.appendChild(document.createElement('div'));
		expect(isRtl(child)).toBe(false);
		document.documentElement.style.direction = 'rtl';
		expect(isRtl(child)).toBe(true);
		expect(isRtl(null)).toBe(false);
		child.remove();
	});
});

describe('inlineKey', () => {
	it('swaps Left and Right in a right-to-left layout and nothing else', () => {
		expect(inlineKey('ArrowLeft', true)).toBe('ArrowRight');
		expect(inlineKey('ArrowRight', true)).toBe('ArrowLeft');
		expect(inlineKey('ArrowDown', true)).toBe('ArrowDown');
		expect(inlineKey('ArrowLeft', false)).toBe('ArrowLeft');
	});
});

describe('overflowSides', () => {
	it('left to right: more to the right at the start, more to the left at the end', () => {
		expect(overflowSides(0, 500, 300, false)).toEqual({ left: false, right: true });
		expect(overflowSides(100, 500, 300, false)).toEqual({ left: true, right: true });
		expect(overflowSides(200, 500, 300, false)).toEqual({ left: true, right: false });
	});

	it('right to left: scrollLeft is 0 at the start on the right and negative towards the left', () => {
		expect(overflowSides(0, 500, 300, true)).toEqual({ left: true, right: false });
		expect(overflowSides(-100, 500, 300, true)).toEqual({ left: true, right: true });
		expect(overflowSides(-200, 500, 300, true)).toEqual({ left: false, right: true });
	});

	it('has nothing to scroll when everything fits', () => {
		expect(overflowSides(0, 300, 300, false)).toEqual({ left: false, right: false });
		expect(overflowSides(0, 300, 300, true)).toEqual({ left: false, right: false });
	});
});

describe('endScrollLeft and wheelScrollDelta', () => {
	it('go towards the end of the line in either direction', () => {
		expect(endScrollLeft(400, false)).toBe(400);
		expect(endScrollLeft(400, true)).toBe(-400);
		expect(wheelScrollDelta(40, false)).toBe(40);
		expect(wheelScrollDelta(40, true)).toBe(-40);
	});
});
