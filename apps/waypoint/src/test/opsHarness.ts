// Shared by the operations tests: a request to submit and a store over a fake with a manual frame clock
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Conflict } from '@liminal-hq/waypoint-protocol/generated/Conflict';
import type { JobRequest } from '@liminal-hq/waypoint-protocol/generated/JobRequest';
import { fileLocation } from '../services/fakeVfsClient';

/** A copy of `names` from `/src` into `destination`, started by `main-1`. */
export function request(names: string[] = ['a.txt'], destination = '/dest'): JobRequest {
	return {
		kind: { kind: 'copy' },
		sources: { kind: 'locations', locations: names.map((n) => fileLocation(`/src/${n}`)) },
		destination: fileLocation(destination),
		name: null,
		options: { conflict: null, verify: null },
		originWindow: 'main-1',
	};
}

/** A clash over `name` in `/dest`: the existing entry against the one coming from `/src`. */
export function conflictFor(name: string, overrides: Partial<Conflict> = {}): Conflict {
	return {
		source: fileLocation(`/src/${name}`),
		existing: fileLocation(`/dest/${name}`),
		name,
		kind: 'fileOverFile',
		withinBatch: false,
		sourceSize: 2048,
		existingSize: 1024,
		sourceModifiedMs: new Date(2026, 5, 2, 14, 30).getTime(),
		existingModifiedMs: new Date(2026, 5, 1, 14, 30).getTime(),
		...overrides,
	};
}
