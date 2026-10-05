// Tests for the Connect dialog's pure rules
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import {
	connectionErrorText,
	draftOf,
	draftProblem,
	emptyForm,
	familyOf,
	formOf,
	formProblem,
	methodsFor,
	questionOf,
	rememberedText,
	withScheme,
} from './connectModel';
import { draft, serverLocation } from './fakeConnectionsClient';

const at = serverLocation('sftp://nas.lan');

describe('the form', () => {
	it('sends blanks as nothing and a number that is not one as a value Rust refuses', () => {
		const sent = draftOf({
			...emptyForm(),
			host: ' nas.lan ',
			port: '22x',
			user: ' ',
			keyFile: 'k',
		});
		expect(sent.host).toBe('nas.lan');
		expect(sent.port).toBe(0);
		expect(sent.user).toBeNull();
		expect(sent.keyFile).toBeNull();
		expect(draftOf({ ...emptyForm(), auth: 'keyFile', keyFile: '~/.ssh/k' }).keyFile).toBe(
			'~/.ssh/k',
		);
		expect(draftOf({ ...emptyForm(), port: '70000' }).port).toBe(0);
	});

	it('keeps the sign-in choice when an address fills the fields', () => {
		const typed = draft({ host: 'nas.lan', user: 'me', port: 2222 });
		const form = formOf(typed, { ...emptyForm(), auth: 'password', name: 'NAS' });
		expect(form).toMatchObject({
			host: 'nas.lan',
			user: 'me',
			port: '2222',
			auth: 'password',
			name: 'NAS',
		});
	});
});

describe('refusals and errors', () => {
	it('puts a refused field under its input', () => {
		expect(
			draftProblem({ kind: 'connections', error: { kind: 'draft', error: { kind: 'port' } } }),
		).toEqual({ field: 'port', message: 'The port is a number from 1 to 65535.' });
		expect(draftProblem({ kind: 'unreachable' })).toBeNull();
	});

	it('words a skewed clock and an archived file', () => {
		expect(connectionErrorText({ kind: 'clockSkew', location: at, skewMs: null })).toMatch(
			/clock is off/,
		);
		expect(connectionErrorText({ kind: 'archived', location: at })).toMatch(/needs a restore/);
	});

	it('words every reason a server could not be reached', () => {
		expect(
			connectionErrorText({ kind: 'unreachable', location: at, reason: 'nameNotResolved' }),
		).toBe('Not connected: the host name was not found.');
		expect(connectionErrorText({ kind: 'unreachable', location: at, reason: 'refused' })).toMatch(
			/refused/,
		);
		expect(connectionErrorText({ kind: 'timeout', location: at })).toMatch(/in time/);
		expect(connectionErrorText(new Error('boom'))).toBe('Not connected: boom');
	});

	it('knows which errors ask a question', () => {
		expect(
			questionOf({ kind: 'authRequired', location: at, prompt: { kind: 'password', user: null } }),
		).toBe('signIn');
		expect(
			questionOf({
				kind: 'hostKeyChanged',
				location: at,
				change: {
					host: 'nas.lan',
					recordedAlgorithm: 'ssh-ed25519',
					recordedFingerprint: 'SHA256:a',
					offeredAlgorithm: 'ssh-ed25519',
					offeredFingerprint: 'SHA256:b',
				},
			}),
		).toBe('hostKeyChanged');
		expect(questionOf({ kind: 'timeout', location: at })).toBeNull();
	});

	it('says what became of Remember', () => {
		expect(rememberedText({ kind: 'no' })).toBeNull();
		expect(rememberedText({ kind: 'sessionOnly', why: 'locked' })).toMatch(/keyring is locked/);
	});
});

describe('a protocol that is turned off', () => {
	it('is said in words with where to turn it on', () => {
		expect(connectionErrorText({ kind: 'protocolOff', scheme: 'sftp' })).toBe(
			'Not connected: SFTP (SSH) is turned off in Settings → Experimental.',
		);
	});

	it('asks no question', () => {
		expect(questionOf({ kind: 'protocolOff', scheme: 'sftp' })).toBeNull();
	});
});

describe('the form of each protocol', () => {
	it('groups the protocols into the families the form is shaped for', () => {
		expect(['sftp', 'smb', 'dav', 'davs', 's3', 'x'].map(familyOf)).toEqual([
			'ssh',
			'smb',
			'dav',
			'dav',
			'other',
			'other',
		]);
	});

	it('offers each protocol only the ways to sign in it has', () => {
		expect(methodsFor('sftp')).toEqual(['auto', 'password', 'keyFile']);
		expect(methodsFor('smb')).toEqual(['auto', 'password']);
		expect(methodsFor('davs')).toEqual(['auto', 'password', 'token']);
		expect(methodsFor('s3')).toEqual(['auto', 'password']);
	});

	it('sends back to automatic a way to sign in that the chosen protocol lacks', () => {
		const form = { ...emptyForm('sftp'), auth: 'keyFile' as const, keyFile: '~/.ssh/id' };
		expect(withScheme(form, 'smb').auth).toBe('auto');
		expect(withScheme({ ...form, auth: 'password' }, 'smb').auth).toBe('password');
		expect(withScheme({ ...form, auth: 'password' }, 'davs').scheme).toBe('davs');
	});

	it('joins an SMB domain to its user, and a share to nothing but the start folder', () => {
		const sent = draftOf({
			...emptyForm('smb'),
			host: 'files.lan',
			domain: ' WORK ',
			user: ' me ',
			startFolder: '/Projects',
			jumpHost: 'bastion',
			auth: 'keyFile',
			keyFile: '/k',
		});
		expect(sent.user).toBe('WORK;me');
		expect(sent.startFolder).toBe('/Projects');
		// What only SSH has is not sent for SMB, so a saved SMB connection never holds it.
		expect([sent.jumpHost, sent.keyFile, sent.auth]).toEqual([null, null, 'auto']);
		expect(draftOf({ ...emptyForm('smb'), host: 'h', user: 'me' }).user).toBe('me');
	});

	it('splits a saved SMB user back into its domain and user', () => {
		const saved = formOf(draft({ scheme: 'smb', host: 'h', user: 'WORK;me' }));
		expect([saved.domain, saved.user]).toEqual(['WORK', 'me']);
		const plain = formOf(draft({ scheme: 'smb', host: 'h', user: 'me' }));
		expect([plain.domain, plain.user]).toEqual(['', 'me']);
		// A semicolon in an SSH user is just a character.
		const ssh = formOf(draft({ scheme: 'sftp', host: 'h', user: 'a;b' }));
		expect([ssh.domain, ssh.user]).toEqual(['', 'a;b']);
	});

	it('carries WebDAV’s sign-in and dialect as options, and only for WebDAV', () => {
		const form = { ...emptyForm('davs'), host: 'h', davAuth: 'digest' as const, nextcloud: true };
		const sent = draftOf(form);
		expect([sent.options.davAuth, sent.options.davPreset]).toEqual(['digest', 'nextcloud']);
		const auto = draftOf({ ...form, davAuth: 'auto', nextcloud: false });
		expect([auto.options.davAuth, auto.options.davPreset]).toEqual([null, null]);
		const ssh = draftOf({ ...form, scheme: 'sftp' });
		expect([ssh.options.davAuth, ssh.options.davPreset]).toEqual([null, null]);
		const back = formOf(
			draft({
				scheme: 'davs',
				host: 'h',
				options: { ...draft().options, davAuth: 'basic', davPreset: 'nextcloud' },
			}),
		);
		expect([back.davAuth, back.nextcloud]).toEqual(['basic', true]);
	});

	it('keeps what a typed address cannot say when it is read over the form', () => {
		const chosen = { ...emptyForm('davs'), davAuth: 'basic' as const, nextcloud: true };
		const read = formOf(draft({ scheme: 'davs', host: 'cloud', user: 'alice' }), chosen);
		expect([read.davAuth, read.nextcloud, read.user]).toEqual(['basic', true, 'alice']);
	});

	it('refuses a domain with no user, and nothing else', () => {
		expect(formProblem({ ...emptyForm('smb'), domain: 'WORK' })).toMatchObject({ field: 'user' });
		expect(formProblem({ ...emptyForm('smb'), domain: 'WORK', user: 'me' })).toBeNull();
		expect(formProblem({ ...emptyForm('sftp'), domain: 'WORK' })).toBeNull();
	});
});
