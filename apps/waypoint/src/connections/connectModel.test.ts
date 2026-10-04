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
	formOf,
	questionOf,
	rememberedText,
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
