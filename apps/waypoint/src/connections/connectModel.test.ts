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
			's3',
			'other',
		]);
	});

	it('offers each protocol only the ways to sign in it has', () => {
		expect(methodsFor('sftp')).toEqual(['auto', 'password', 'keyFile']);
		expect(methodsFor('smb')).toEqual(['auto', 'password']);
		expect(methodsFor('davs')).toEqual(['auto', 'password', 'token']);
		// An S3 login is an access key (an id and a secret), not a way to sign in.
		expect(methodsFor('s3')).toEqual(['auto']);
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

describe('the S3 form', () => {
	const s3 = (fields: Partial<ReturnType<typeof emptyForm>> = {}) => ({
		...emptyForm('s3'),
		host: 'Photos',
		user: 'AKIA',
		...fields,
	});

	it('sends the bucket as the host, the key id as the user, and the endpoint a preset makes', () => {
		const sent = draftOf(
			s3({ s3Preset: 'b2', s3Value: 'us-west-004', port: '9', s3Region: ' us-west-004 ' }),
		);
		expect(sent).toMatchObject({
			scheme: 's3',
			host: 'Photos',
			user: 'AKIA',
			port: null,
			auth: 'auto',
		});
		expect(sent.options).toMatchObject({
			s3Endpoint: 'https://s3.us-west-004.backblazeb2.com',
			s3Preset: 'b2',
			s3Region: 'us-west-004',
			s3PathStyle: null,
		});
		expect(draftOf(s3()).options.s3Endpoint).toBeNull();
		expect(draftOf({ ...s3(), scheme: 'sftp' }).options).toMatchObject({
			s3Endpoint: null,
			s3Preset: null,
		});
	});

	it('reads a saved connection back into its service and input', () => {
		const saved = draft({
			scheme: 's3',
			host: 'media',
			user: 'AKIA',
			options: {
				...draft().options,
				s3Endpoint: 'https://abc123.r2.cloudflarestorage.com',
				s3Preset: 'r2',
				s3PathStyle: false,
				s3Region: 'auto',
			},
		});
		expect(formOf(saved)).toMatchObject({
			s3Preset: 'r2',
			s3Value: 'abc123',
			s3PathStyle: false,
			s3Region: 'auto',
			host: 'media',
			user: 'AKIA',
		});
		// What a typed address cannot say stays as it was chosen.
		const typed = draft({ scheme: 's3', host: 'media' });
		expect(formOf(typed, s3({ s3Region: 'eu-west-1', s3PathStyle: true }))).toMatchObject({
			s3Region: 'eu-west-1',
			s3PathStyle: true,
		});
	});

	it('names the bucket or the service input that is missing', () => {
		expect(formProblem(s3({ host: ' ' }))).toMatchObject({ field: 'host' });
		expect(formProblem(s3({ s3Preset: 'r2' }))).toMatchObject({ field: 's3Value' });
		expect(formProblem(s3({ s3Preset: 'custom', s3Value: 'https://x.test' }))).toBeNull();
		expect(formProblem(s3())).toBeNull();
	});

	it('puts what Rust refuses under its field', () => {
		const refused = (kind: 'endpoint' | 'region') => ({
			kind: 'connections' as const,
			error: { kind: 'draft' as const, error: { kind } },
		});
		expect(draftProblem(refused('endpoint'))).toMatchObject({ field: 's3Value' });
		expect(draftProblem(refused('region'))).toMatchObject({ field: 's3Region' });
	});
});

describe('previews of a server’s files', () => {
	it('start off, and carry the choice and the size cap both ways', () => {
		expect(emptyForm().thumbnails).toBe('off');
		const sent = draftOf({ ...emptyForm(), thumbnails: 'always', thumbnailMaxMb: '8' });
		expect(sent.options.thumbnails).toBe('always');
		expect(sent.options.thumbnailMaxMb).toBe(8);
		expect(draftOf({ ...emptyForm(), thumbnailMaxMb: 'lots' }).options.thumbnailMaxMb).toBe(0);
		expect(draftOf(emptyForm()).options.thumbnailMaxMb).toBeNull();
		const back = formOf(draft({ options: sent.options }));
		expect([back.thumbnails, back.thumbnailMaxMb]).toEqual(['always', '8']);
	});

	it('puts a refused size cap under its own field', () => {
		const problem = draftProblem({
			kind: 'connections',
			error: { kind: 'draft', error: { kind: 'option', option: 'thumbnailMaxMb' } },
		});
		expect(problem?.field).toBe('thumbnailMaxMb');
	});
});
