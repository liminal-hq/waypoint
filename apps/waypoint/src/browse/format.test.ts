// Verifies size and date formatting through `Intl`
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { formatModified, formatSize } from './format';

describe('formatSize', () => {
	it('uses the largest unit under 1000', () => {
		expect(formatSize(0, 'en-CA')).toBe('0 bytes');
		expect(formatSize(999, 'en-CA')).toBe('999 bytes');
		expect(formatSize(1000, 'en-CA')).toBe('1 kB');
		expect(formatSize(1_500_000, 'en-CA')).toBe('1.5 MB');
		expect(formatSize(12_345_678_901, 'en-CA')).toBe('12 GB');
	});

	it('promotes a unit when rounding reaches 1000', () => {
		expect(formatSize(999_499, 'en-CA')).toBe('999 kB');
		expect(formatSize(999_500, 'en-CA')).toBe('1 MB');
		expect(formatSize(999_999, 'en-CA')).toBe('1 MB');
		expect(formatSize(999_999_999, 'en-CA')).toBe('1 GB');
		expect(formatSize(9_949, 'en-CA')).toBe('9.9 kB');
		expect(formatSize(9_950, 'en-CA')).toBe('10 kB');
	});

	it('follows the locale', () => {
		expect(formatSize(1500, 'fr-CA')).toContain('1,5');
		expect(formatSize(1500, 'en-CA')).toContain('1.5');
	});
});

describe('formatModified', () => {
	it('formats a timestamp with a date and a time in the locale', () => {
		const text = formatModified(Date.UTC(2026, 0, 2, 15, 4), 'en-CA');
		expect(text).toMatch(/2026/);
		expect(text).toMatch(/\d{1,2}:\d{2}/);
	});
});

describe('formatModified with an hour cycle', () => {
	// 13:05 local time, built from parts so the test does not depend on the machine's time zone.
	const afternoon = new Date(2026, 9, 1, 13, 5).getTime();

	it('shows a 24-hour clock for h23, whatever the locale defaults to', () => {
		expect(formatModified(afternoon, 'en-CA', 'h23')).toContain('13:05');
		expect(formatModified(afternoon, 'en-CA', 'h23')).not.toMatch(/p\.m\./);
	});

	it('shows a 12-hour clock for h12', () => {
		expect(formatModified(afternoon, 'en-CA', 'h12')).toMatch(/1:05\s*p\.m\./);
	});

	it('leaves the choice to the locale without an hour cycle', () => {
		expect(formatModified(afternoon, 'en-CA')).toMatch(/1:05\s*p\.m\./);
		expect(formatModified(afternoon, 'en-GB')).toContain('13:05');
	});

	it('keeps a separate formatter for each hour cycle', () => {
		const a = formatModified(afternoon, 'en-CA', 'h23');
		const b = formatModified(afternoon, 'en-CA', 'h12');
		const c = formatModified(afternoon, 'en-CA', 'h23');
		expect(a).not.toBe(b);
		expect(c).toBe(a);
	});
});
