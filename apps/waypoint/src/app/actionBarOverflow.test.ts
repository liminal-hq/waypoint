// Tests for how many Action bar buttons fit before the rest collapse into More
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { fitCount } from './actionBarOverflow';

describe('fitCount', () => {
	const widths = [80, 60, 60, 60, 80];

	it('shows everything, with no More button, when it all fits', () => {
		// 340 of buttons plus 4 gaps of 2.
		expect(fitCount(widths, 348, 2, 36)).toBe(5);
		expect(fitCount(widths, 1000, 2, 36)).toBe(5);
	});

	it('collapses the trailing buttons into More when they do not', () => {
		// More (36) + 80 + 2 + 60 + 2 + 60 = 240 fits in 250; one more would be 302.
		expect(fitCount(widths, 250, 2, 36)).toBe(3);
		expect(fitCount(widths, 347, 2, 36)).toBe(4);
	});

	it('keeps room for More itself', () => {
		// More (36) + gap (2) + the first button (80) is 118.
		expect(fitCount(widths, 117, 2, 36)).toBe(0);
		expect(fitCount(widths, 118, 2, 36)).toBe(1);
		expect(fitCount(widths, 179, 2, 36)).toBe(1);
		expect(fitCount(widths, 180, 2, 36)).toBe(2);
	});

	it('shows none when not even one button fits beside More', () => {
		expect(fitCount(widths, 40, 2, 36)).toBe(0);
		expect(fitCount(widths, 0, 2, 36)).toBe(0);
	});

	it('handles no buttons', () => {
		expect(fitCount([], 0, 2, 36)).toBe(0);
	});
});
