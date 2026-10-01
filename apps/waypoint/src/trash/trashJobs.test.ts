// Verifies the words for why a Trash job failed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { opsErrorText } from './trashJobs';

const at = { display: '/home/a/x', uri: 'file:///home/a/x' };

describe('opsErrorText', () => {
	it('names the place for the errors that have one', () => {
		expect(opsErrorText({ kind: 'notFound', location: at })).toBe('/home/a/x was not found');
		expect(opsErrorText({ kind: 'permissionDenied', location: at })).toBe(
			'permission denied for /home/a/x',
		);
		expect(opsErrorText({ kind: 'nameInUse', location: at })).toBe('/home/a/x already exists');
		expect(opsErrorText({ kind: 'cannotReplace', location: at })).toBe(
			'/home/a/x cannot be replaced by an item of another kind',
		);
	});

	it('passes on the Trash’s and the system’s own words', () => {
		expect(opsErrorText({ kind: 'trashUnavailable', reason: 'no Trash here' })).toBe(
			'no Trash here',
		);
		expect(opsErrorText({ kind: 'io', message: 'the disk hiccupped' })).toBe('the disk hiccupped');
		expect(opsErrorText({ kind: 'unsupported', what: 'restoring to another place' })).toBe(
			'restoring to another place',
		);
		expect(opsErrorText({ kind: 'originMissingParent', location: at })).toMatch(
			/\/home\/a\/x no longer exists/,
		);
	});

	it('has something to say for an error with no words of its own', () => {
		expect(opsErrorText({ kind: 'cancelled' })).toBe('something went wrong');
	});
});
