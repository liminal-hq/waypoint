// Verifies the plain-language message for every kind of error an item can stop a job with, and the decisions offered
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import { describe, expect, it } from 'vitest';
import { decisionsFor, problemText } from './problemModel';

const at = { display: '/home/a/x.txt', uri: 'file:///home/a/x.txt' };

const CASES: Array<[OpsError, RegExp]> = [
	[{ kind: 'notFound', location: at }, /\/home\/a\/x\.txt was not found/],
	[{ kind: 'permissionDenied', location: at }, /Permission denied for \/home\/a\/x\.txt/],
	[{ kind: 'notEnoughSpace', needed: 5_000_000, free: 1_000_000 }, /5 MB needed, 1 MB free/],
	[{ kind: 'notEnoughSpace', needed: 0, free: 0 }, /not enough free space on the destination/],
	[{ kind: 'invalidName', name: 'a:b', reason: 'contains a colon' }, /“a:b” cannot be used.*colon/],
	[{ kind: 'nameInUse', location: at }, /\/home\/a\/x\.txt is already taken/],
	[{ kind: 'sameFolder' }, /already in that folder/],
	[{ kind: 'intoItself' }, /cannot be put inside itself/],
	[{ kind: 'protected', location: at }, /is protected/],
	[{ kind: 'trashUnavailable', reason: 'no Trash' }, /Trash is not available: no Trash/],
	[{ kind: 'originMissingParent', location: at }, /no longer exists. Recreate it/],
	[{ kind: 'cancelled' }, /was cancelled/],
	[{ kind: 'unsupported', what: 'links' }, /not supported yet: links/],
	[
		{ kind: 'protocolOff', scheme: 'davs' },
		/WebDAV \(HTTPS\) is turned off in Settings → Experimental/,
	],
	[{ kind: 'changedSince', location: at }, /changed after the job was planned/],
	[
		{ kind: 'verifyFailed', location: at, expected: 'aa', actual: 'bb' },
		/did not read back the same/,
	],
	[{ kind: 'cannotReplace', location: at }, /entry of another kind/],
	[
		{ kind: 'archiveLimit', location: at, limit: { kind: 'entries', found: 5, max: 2 } },
		/larger than the archive limits allow/,
	],
	[
		{ kind: 'undoStale', location: at, reason: 'missing' },
		/cannot be undone.*no longer where it was/,
	],
	[{ kind: 'undoUnavailable', reason: 'nothing to undo' }, /nothing to undo/],
	[{ kind: 'io', message: 'the disk hiccupped' }, /system reported a problem: the disk hiccupped/],
	[
		{
			kind: 'connection',
			error: {
				kind: 'disconnected',
				location: { display: 'sftp://nas/srv', uri: 'sftp://nas/srv' },
			},
		},
		/server that holds sftp:\/\/nas\/srv failed\. Retry connects again/,
	],
];

describe('problemText', () => {
	it.each(CASES)('words %j', (error, message) => {
		expect(problemText(error).message).toMatch(message);
	});

	it('covers every kind of error', () => {
		const kinds = new Set(CASES.map(([error]) => error.kind));
		expect(kinds.size).toBe(21);
	});

	it('keeps the checksums of a failed verification as details, not in the message', () => {
		const { message, details } = problemText({
			kind: 'verifyFailed',
			location: at,
			expected: 'aa11',
			actual: 'bb22',
		});
		expect(message).not.toMatch(/aa11|bb22/);
		expect(details).toEqual(['Expected checksum: aa11', 'Found checksum: bb22']);
	});

	it('says which archive limit was passed, in details', () => {
		const detail = (limit: Parameters<typeof problemText>[0] & { kind: 'archiveLimit' }) =>
			problemText(limit).details;
		expect(
			detail({ kind: 'archiveLimit', location: at, limit: { kind: 'entries', found: 5, max: 2 } }),
		).toEqual(['It holds 5 entries; the limit is 2.']);
		expect(
			detail({
				kind: 'archiveLimit',
				location: at,
				limit: { kind: 'ratio', ratio: 5000, max: 1000 },
			}),
		).toEqual(['It expands to 5000 times its own size; the limit is 1000 times.']);
		expect(
			detail({
				kind: 'archiveLimit',
				location: at,
				limit: { kind: 'bytes', found: 5_000_000, max: 1_000_000 },
			})[0],
		).toMatch(/expands to 5 MB; the limit is 1 MB/);
	});

	it('has no details for the others', () => {
		expect(problemText({ kind: 'io', message: 'x' }).details).toEqual([]);
	});
});

describe('decisionsFor', () => {
	it('offers retry, skip, skip all and cancel', () => {
		expect(decisionsFor({ kind: 'io', message: 'x' })).toEqual([
			'retry',
			'skip',
			'skipAll',
			'cancel',
		]);
	});

	it('adds recreating folders first when the original folder is gone', () => {
		expect(decisionsFor({ kind: 'originMissingParent', location: at })).toEqual([
			'createParents',
			'retry',
			'skip',
			'skipAll',
			'cancel',
		]);
	});
});
