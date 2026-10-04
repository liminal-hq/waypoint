// Tests for connecting with questions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AnswerInput } from '@liminal-hq/waypoint-protocol/generated/AnswerInput';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { describe, expect, it, vi } from 'vitest';
import { connectAnswering } from './connectFlow';
import { serverLocation } from './fakeConnectionsClient';

const at = serverLocation('sftp://nas.lan');
const unknownKey: VfsError = {
	kind: 'hostKeyUnknown',
	location: at,
	key: { host: 'nas.lan', algorithm: 'ssh-ed25519', fingerprint: 'SHA256:abc' },
};
const password: VfsError = {
	kind: 'authRequired',
	location: at,
	prompt: { kind: 'password', user: 'me' },
};

describe('connectAnswering', () => {
	it('asks each question in turn and sends each answer with the next attempt', async () => {
		const seen: (AnswerInput | null)[] = [];
		const attempt = vi.fn(async (answer: AnswerInput | null) => {
			seen.push(answer);
			if (seen.length === 1) throw unknownKey;
			if (seen.length === 2) throw password;
			return 'ok';
		});
		const ask = vi.fn(async (error: VfsError) =>
			error.kind === 'hostKeyUnknown'
				? {
						answer: {
							kind: 'trustHostKey',
							fingerprint: 'SHA256:abc',
							remember: true,
						} as AnswerInput,
						remember: false,
					}
				: {
						answer: { kind: 'password', user: 'me', password: 'pw' } as AnswerInput,
						remember: true,
					},
		);
		const outcome = await connectAnswering(attempt, ask);
		expect(outcome).toEqual({ kind: 'connected', value: 'ok' });
		expect(seen.map((answer) => answer?.kind ?? null)).toEqual([null, 'trustHostKey', 'password']);
		expect(attempt).toHaveBeenLastCalledWith(expect.objectContaining({ kind: 'password' }), true);
	});

	it('stops when the person cancels, and reports an error that asks nothing', async () => {
		expect(
			await connectAnswering(
				async () => {
					throw unknownKey;
				},
				async () => null,
			),
		).toEqual({ kind: 'cancelled' });
		const refused: VfsError = { kind: 'unreachable', location: at, reason: 'refused' };
		const ask = vi.fn();
		expect(
			await connectAnswering(async () => {
				throw refused;
			}, ask),
		).toEqual({ kind: 'failed', error: refused });
		expect(ask).not.toHaveBeenCalled();
	});

	it('gives up on a server that never stops asking', async () => {
		const outcome = await connectAnswering(
			async () => {
				throw password;
			},
			async () => ({ answer: { kind: 'password', user: null, password: 'x' }, remember: false }),
		);
		expect(outcome).toEqual({ kind: 'failed', error: password });
	});
});
