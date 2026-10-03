// Verifies the permissions are taken apart right and times are written out in full
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	formatFullTime,
	formatIsoTime,
	octalMode,
	permissionRows,
	specialBits,
} from './propertiesDetailModel';

describe('permissionRows', () => {
	it('splits 0755 into owner, group and others', () => {
		expect(permissionRows(0o755)).toEqual([
			{ who: 'owner', read: true, write: true, execute: true },
			{ who: 'group', read: true, write: false, execute: true },
			{ who: 'others', read: true, write: false, execute: true },
		]);
	});

	it('shows 0600 as the owner only', () => {
		expect(permissionRows(0o600).map((row) => [row.read, row.write, row.execute])).toEqual([
			[true, true, false],
			[false, false, false],
			[false, false, false],
		]);
	});
});

describe('specialBits', () => {
	it('lists setuid, setgid and sticky in the order ls shows them', () => {
		expect(specialBits(0o7755)).toEqual(['setuid', 'setgid', 'sticky']);
		expect(specialBits(0o1777)).toEqual(['sticky']);
		expect(specialBits(0o644)).toEqual([]);
	});
});

describe('octalMode', () => {
	it('pads to four digits and keeps the special one', () => {
		expect(octalMode(0o644)).toBe('0644');
		expect(octalMode(0o104755)).toBe('4755');
	});
});

describe('times', () => {
	it('writes ISO 8601 in UTC', () => {
		expect(formatIsoTime(Date.UTC(2026, 9, 2, 14, 5, 9))).toBe('2026-10-02T14:05:09.000Z');
	});

	it('writes the full date and the seconds, in the 12 or 24-hour clock asked for', () => {
		const ms = Date.UTC(2026, 9, 2, 15, 5, 9);
		expect(formatFullTime(ms, 'h23', 'en-CA')).toMatch(/:\d{2}:09/);
		expect(formatFullTime(ms, 'h12', 'en-CA')).toMatch(/[ap]\.?m\.?/i);
		expect(formatFullTime(ms, 'h23', 'en-CA')).not.toMatch(/[ap]\.?m\.?/i);
	});
});
