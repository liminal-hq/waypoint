// Tests for the development timing harness's pure parts and its install hook
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { installPerfHarness, stats } from './perfHarness';

describe('stats', () => {
	it('reports percentiles of a sample', () => {
		const sample = Array.from({ length: 100 }, (_, i) => i + 1);
		expect(stats(sample)).toEqual({ n: 100, p50: 51, p95: 96, p99: 100, max: 100 });
	});

	it('does not reorder the sample it is given', () => {
		const sample = [3, 1, 2];
		stats(sample);
		expect(sample).toEqual([3, 1, 2]);
	});

	it('handles an empty sample', () => {
		expect(stats([])).toEqual({ n: 0 });
	});
});

describe('installPerfHarness', () => {
	it('exposes the harness on the window', () => {
		installPerfHarness();
		const harness = window.__waypointPerf as Record<string, unknown>;
		expect(Object.keys(harness).sort()).toEqual([
			'autoRun',
			'jumps',
			'runAll',
			'selectAll',
			'sortBy',
			'stats',
			'sweep',
		]);
	});
});
