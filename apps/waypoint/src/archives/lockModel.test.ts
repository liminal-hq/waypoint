// Verifies how an archive's password question is told from a server's, and found in a failed job
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { describe, expect, it } from 'vitest';
import { commandErrorMessage, lockOf } from './askPassphrase';
import { isArchiveLock, lockedName } from './lockModel';

const archive = { display: '/home/me/a.zip', uri: 'archive:file:///home/me/a.zip!/' };
const server = { display: 'sftp://me@nas/key', uri: 'sftp://me@nas.lan/' };

const required = (location: typeof archive): VfsError => ({
	kind: 'authRequired',
	location,
	prompt: { kind: 'passphrase', subject: 'a.zip' },
});

describe('an archive lock', () => {
	it('is a password asked at an archive, and not a server login', () => {
		expect(isArchiveLock(required(archive))).toBe(true);
		expect(isArchiveLock({ kind: 'authFailed', location: archive })).toBe(true);
		expect(isArchiveLock(required(server))).toBe(false);
		expect(isArchiveLock({ kind: 'authFailed', location: server })).toBe(false);
		expect(isArchiveLock({ kind: 'notFound', location: archive })).toBe(false);
	});

	it('is named by the file, however the location is written', () => {
		expect(lockedName(archive)).toBe('a.zip');
		expect(lockedName({ display: 'C:\\Users\\me\\a.zip', uri: '' })).toBe('a.zip');
		expect(lockedName({ display: 'a.zip › docs', uri: '' })).toBe('docs');
	});
});

describe('the lock a failed command holds', () => {
	const connection = (error: VfsError) => ({ kind: 'connection' as const, error });

	it('is found in a plan that was refused and in a job that failed', () => {
		const lock = required(archive);
		expect(lockOf({ kind: 'ops', message: 'x', error: connection(lock) })).toEqual(lock);
		expect(lockOf(connection(lock))).toEqual(lock);
		expect(lockOf(lock)).toEqual(lock);
	});

	it('is not found in another failure, or in a server’s own question', () => {
		expect(lockOf(connection(required(server)))).toBeNull();
		expect(lockOf({ kind: 'ops', error: { kind: 'io', message: 'x' } })).toBeNull();
		expect(lockOf(null)).toBeNull();
		expect(lockOf(new Error('x'))).toBeNull();
	});

	it('words a refusal to give the password', () => {
		expect(commandErrorMessage({ kind: 'unsupported', what: 'archives in this build' })).toBe(
			'archives in this build',
		);
		expect(commandErrorMessage(new Error('the command is gone'))).toBe('the command is gone');
		expect(commandErrorMessage({ kind: 'io', message: 'x' })).toBe(
			'The password could not be given to the archive.',
		);
	});
});
