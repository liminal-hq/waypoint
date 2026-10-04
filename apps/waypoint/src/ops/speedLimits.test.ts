// Verifies the speed limits a row offers and how they are worded
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { speedLimitChoices, speedLimitLabel } from './speedLimits';

describe('speed limit choices', () => {
	it('offer no limit first and the usual speeds in order', () => {
		const choices = speedLimitChoices(null);
		expect(choices[0]).toBeNull();
		expect(choices.slice(1)).toEqual([1, 2, 5, 10, 25, 50, 100].map((mb) => mb * 1_000_000));
	});

	it('keep a limit another window set, in its place', () => {
		const choices = speedLimitChoices(7_500_000);
		expect(choices).toContain(7_500_000);
		expect(choices.indexOf(7_500_000)).toBe(choices.indexOf(5_000_000) + 1);
		expect(speedLimitChoices(10_000_000)).toHaveLength(8);
	});
});

describe('speed limit words', () => {
	it('say no limit, whole megabytes and a decimal for the rest', () => {
		expect(speedLimitLabel(null)).toBe('No limit');
		expect(speedLimitLabel(10_000_000)).toBe('10 MB/s');
		expect(speedLimitLabel(7_500_000)).toBe('7.5 MB/s');
	});
});
