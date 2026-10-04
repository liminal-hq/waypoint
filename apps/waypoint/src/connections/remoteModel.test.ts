// Tests for the words and actions of a remote location's state
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { serverLocation } from './fakeConnectionsClient';
import { isConnectionError, remoteStateText, stateTone, stateWords } from './remoteModel';

const at = serverLocation('sftp://nas.lan');

describe('remote states', () => {
	it('tells connection errors from folder errors', () => {
		expect(isConnectionError({ kind: 'timeout', location: at })).toBe(true);
		expect(isConnectionError({ kind: 'notFound', location: at })).toBe(false);
	});

	it('puts every state in words as well as a tone', () => {
		expect(stateWords({ kind: 'connected' })).toBe('Connected');
		expect(stateWords({ kind: 'idle' })).toBe('Not connected');
		const offline = {
			kind: 'failed' as const,
			error: { kind: 'unreachable' as const, location: at, reason: 'offline' as const },
		};
		expect(stateTone(offline)).toBe('offline');
		expect(stateWords(offline)).toBe('Offline');
		expect(
			stateWords({
				kind: 'failed',
				error: { kind: 'authRequired', location: at, prompt: { kind: 'password', user: null } },
			}),
		).toBe('Sign-in needed');
	});

	it('offers Reconnect, Sign In or Review by the error', () => {
		expect(remoteStateText({ kind: 'disconnected', location: at }).action).toBe('reconnect');
		expect(remoteStateText({ kind: 'authFailed', location: at }).action).toBe('signIn');
		expect(
			remoteStateText({
				kind: 'hostKeyUnknown',
				location: at,
				key: { host: 'nas.lan', algorithm: 'ssh-ed25519', fingerprint: 'SHA256:x' },
			}).action,
		).toBe('review');
		expect(remoteStateText({ kind: 'unreachable', location: at, reason: 'refused' }).detail).toBe(
			'remote.unreachable.refused',
		);
	});
});
