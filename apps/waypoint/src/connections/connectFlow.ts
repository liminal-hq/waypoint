// Connecting with questions: try, and while the server asks something (a login, a host key), ask the person and try again with the answer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AnswerInput } from '@liminal-hq/waypoint-protocol/generated/AnswerInput';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { isVfsError } from '../services/vfsClient';
import { questionOf } from './connectModel';

/** The person's answer to one question, and whether to remember a credential in it. */
export interface Answered {
	answer: AnswerInput;
	remember: boolean;
}

/** Asks the person the question `error` holds; `null` when they cancel. */
export type Ask = (error: VfsError) => Promise<Answered | null>;

export type FlowOutcome<T> =
	{ kind: 'connected'; value: T } | { kind: 'cancelled' } | { kind: 'failed'; error: unknown };

/** More questions than this in one go is a server that keeps asking; the last error is reported. */
const MAX_QUESTIONS = 6;

/**
 * Runs `attempt` with no answer (or `first`), and while it fails with a question asks it and tries
 * again with the answer. Each answer is sent once, with the attempt that carries it, and is not kept
 * here. Ends connected, cancelled by the person, or failed with an error that asks nothing (the
 * server cannot be reached, say), which the caller words.
 */
export async function connectAnswering<T>(
	attempt: (answer: AnswerInput | null, remember: boolean) => Promise<T>,
	ask: Ask,
	first: Answered | null = null,
): Promise<FlowOutcome<T>> {
	let current = first;
	for (let asked = 0; ; asked++) {
		try {
			return {
				kind: 'connected',
				value: await attempt(current?.answer ?? null, current?.remember ?? false),
			};
		} catch (error) {
			if (!isVfsError(error) || questionOf(error) === null || asked >= MAX_QUESTIONS) {
				return { kind: 'failed', error };
			}
			const answered = await ask(error);
			if (answered === null) return { kind: 'cancelled' };
			current = answered;
		}
	}
}
