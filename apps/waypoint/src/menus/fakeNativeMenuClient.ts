// An in-memory NativeMenuClient that records the menus it is asked for and answers from a script
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { NativeMenuClient, NativeMenuItem, NativeMenuPoint } from './nativeMenuClient';

export interface FakeNativeMenuCall {
	items: NativeMenuItem[];
	at: NativeMenuPoint | null;
}

export interface FakeNativeMenuClient extends NativeMenuClient {
	calls: FakeNativeMenuCall[];
	/** What the next calls answer: an id, `null` for a dismissed menu, or an error to reject with. Repeats the last. */
	answer(...next: Array<string | null | Error>): void;
}

/** Answers `null` (dismissed) until `answer` says otherwise. */
export function createFakeNativeMenuClient(): FakeNativeMenuClient {
	const calls: FakeNativeMenuCall[] = [];
	let script: Array<string | null | Error> = [null];
	return {
		calls,
		answer(...next) {
			script = next.length > 0 ? next : [null];
		},
		show(items, at) {
			calls.push({ items, at });
			const next = script.length > 1 ? script.shift() : script[0];
			return next instanceof Error ? Promise.reject(next) : Promise.resolve(next ?? null);
		},
	};
}
