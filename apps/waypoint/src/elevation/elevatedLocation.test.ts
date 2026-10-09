// Verifies the mapping between a file location and its elevated form matches the Rust path's URI rules
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	elevatedLocation,
	isElevatedLocation,
	isElevatedUri,
	toElevatedUri,
	toFileUri,
	unelevatedLocation,
} from './elevatedLocation';

describe('the elevated URI mapping', () => {
	it('is the file URI with the elevated scheme, both ways', () => {
		expect(toElevatedUri('file:///etc')).toBe('admin:///etc');
		expect(toFileUri('admin:///etc')).toBe('file:///etc');
	});

	it('keeps percent-encoding as it is', () => {
		expect(toElevatedUri('file:///etc/a%20b%25')).toBe('admin:///etc/a%20b%25');
		expect(toFileUri('admin:///etc/a%20b%25')).toBe('file:///etc/a%20b%25');
	});

	it('maps a Windows drive URI', () => {
		expect(toElevatedUri('file:///C:/Users/a%20b')).toBe('admin:///C:/Users/a%20b');
		expect(toFileUri('admin:///C:/Users/a%20b')).toBe('file:///C:/Users/a%20b');
	});

	it('reads the scheme in any letter case', () => {
		expect(isElevatedUri('ADMIN:///etc')).toBe(true);
		expect(toFileUri('ADMIN:///etc')).toBe('file:///etc');
		expect(toElevatedUri('FILE:///etc')).toBe('admin:///etc');
	});

	it('refuses every other scheme', () => {
		expect(() => toElevatedUri('sftp://me@nas.lan/etc')).toThrow(RangeError);
		expect(() => toElevatedUri('admin:///etc')).toThrow(RangeError);
		expect(() => toFileUri('file:///etc')).toThrow(RangeError);
		expect(() => toFileUri('trash:/')).toThrow(RangeError);
	});

	it('does not take a longer scheme for the elevated one', () => {
		expect(isElevatedUri('administrator:///x')).toBe(false);
		expect(isElevatedUri('archive:file:///a.zip!/')).toBe(false);
	});

	it('maps a location and keeps its display', () => {
		const home = { display: '/home/a b', uri: 'file:///home/a%20b' };
		const admin = elevatedLocation(home);
		expect(admin).toEqual({ display: '/home/a b', uri: 'admin:///home/a%20b' });
		expect(isElevatedLocation(admin)).toBe(true);
		expect(isElevatedLocation(home)).toBe(false);
		expect(isElevatedLocation(undefined)).toBe(false);
		expect(unelevatedLocation(admin)).toEqual(home);
	});
});
