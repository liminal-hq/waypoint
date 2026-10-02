// Verifies the TypeScript mirror of `validate_name` against the table Rust's own test reads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import table from '../../../../crates/waypoint-vfs/tests/fixtures/name_rules.json';
import { checkName, hostNameRule, isReservedName } from './nameRules';

describe('the shared name table', () => {
	it('is judged the same under both rules as `validate_name` judges it', () => {
		expect(table.length).toBeGreaterThan(20);
		for (const { name, sensitive, insensitive } of table) {
			expect(checkName(name, 'sensitive') === null, `${JSON.stringify(name)} sensitive`).toBe(
				sensitive,
			);
			expect(checkName(name, 'insensitive') === null, `${JSON.stringify(name)} insensitive`).toBe(
				insensitive,
			);
		}
	});

	it('counts bytes on Linux and UTF-16 units on Windows, as Rust does', () => {
		expect(checkName('x'.repeat(255), 'sensitive')).toBeNull();
		expect(checkName('x'.repeat(256), 'sensitive')).toEqual({ kind: 'tooLong', limit: 255 });
		expect(checkName('x'.repeat(256), 'insensitive')).toEqual({ kind: 'tooLong', limit: 255 });
		const wide = 'é'.repeat(128); // 256 bytes, 128 units
		expect(checkName(wide, 'sensitive')).toEqual({ kind: 'tooLong', limit: 255 });
		expect(checkName(wide, 'insensitive')).toBeNull();
	});
});

describe('the problems', () => {
	it('name what is wrong, in the order Rust checks', () => {
		expect(checkName('', 'sensitive')).toEqual({ kind: 'empty' });
		expect(checkName('..', 'sensitive')).toEqual({ kind: 'dots' });
		expect(checkName('a\0b', 'sensitive')).toEqual({ kind: 'nul' });
		expect(checkName('a/b', 'sensitive')).toEqual({ kind: 'slash' });
		expect(checkName('a\\b', 'insensitive')).toEqual({ kind: 'backslash' });
		expect(checkName('a?b', 'insensitive')).toEqual({ kind: 'forbidden', character: '?' });
		expect(checkName('a\tb', 'insensitive')).toEqual({ kind: 'forbidden', character: '\t' });
		expect(checkName('end.', 'insensitive')).toEqual({ kind: 'trailing' });
		expect(checkName('NUL.txt', 'insensitive')).toEqual({ kind: 'reserved' });
	});

	it('knows the reserved device names, with an extension and trailing spaces, and COM10 is not one', () => {
		for (const name of ['CON', 'prn', 'Aux.tar.gz', 'nul', 'COM1', 'lpt9', 'com1 ']) {
			expect(isReservedName(name), name).toBe(true);
		}
		for (const name of ['COM10', 'console', 'COM0', 'LPT', 'a.con']) {
			expect(isReservedName(name), name).toBe(false);
		}
	});

	it('takes the rule from the host: Windows names compare without case', () => {
		const agent = navigator.userAgent;
		expect(hostNameRule()).toBe(/windows/i.test(agent) ? 'insensitive' : 'sensitive');
	});
});
