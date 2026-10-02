// Tests reading an element's direction and swapping the arrow keys for it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { inlineKey, isRtl } from './direction';

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
