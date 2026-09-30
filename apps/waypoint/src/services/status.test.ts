// Verifies status aggregation reports a failing plugin as unavailable without hiding the rest
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { collectStatuses } from './status';

describe('collectStatuses', () => {
	it('returns each source under its name', async () => {
		const result = await collectStatuses({
			trash: async () => ({ available: true, reason: null, features: ['restore'] }),
		});
		expect(result.trash).toEqual({ available: true, reason: null, features: ['restore'] });
	});

	it('reports a throwing source as unavailable and keeps the others', async () => {
		const result = await collectStatuses({
			good: async () => ({ available: true, reason: null, features: [] }),
			bad: async () => {
				throw new Error('permission denied');
			},
		});
		expect(result.good?.available).toBe(true);
		expect(result.bad).toEqual({ available: false, reason: 'permission denied', features: [] });
	});
});
