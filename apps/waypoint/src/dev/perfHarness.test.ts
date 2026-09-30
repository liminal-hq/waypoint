// Tests for the development timing harness's pure parts and its install hook
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it, vi } from 'vitest';
import { installPerfHarness, stats, targetRendered } from './perfHarness';

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

describe('installPerfHarness with a malformed hash', () => {
	afterEach(() => {
		window.location.hash = '';
		vi.restoreAllMocks();
	});

	it('logs instead of throwing', () => {
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		window.location.hash = '#perf-auto=%';
		expect(() => installPerfHarness()).not.toThrow();
		expect(console.warn).toHaveBeenCalled();
	});
});

describe('targetRendered', () => {
	it('is false while the old rows are still in the DOM, true once the target row is', () => {
		const box = document.createElement('div');
		const row = (index: number) => {
			const element = document.createElement('div');
			element.setAttribute('role', 'option');
			element.setAttribute('aria-posinset', String(index + 1));
			box.append(element);
		};
		row(0);
		row(1);
		expect(targetRendered(box, 5000)).toBe(false);
		row(5000);
		expect(targetRendered(box, 5000)).toBe(true);
	});
});

describe('openFolder', () => {
	afterEach(() => {
		document.body.innerHTML = '';
		vi.resetModules();
		vi.doUnmock('../services/tabsApi');
		vi.doUnmock('../services/tauriVfsClient');
	});

	it('navigates to the location Rust parses and waits for the new list, not the old one', async () => {
		vi.useFakeTimers();
		const old = document.body.appendChild(document.createElement('div'));
		old.setAttribute('role', 'listbox');
		old.setAttribute('aria-rowcount', '5000');
		const parsed = { display: '/a#b', uri: 'file:///a%23b' };
		const navigate = vi.fn().mockResolvedValue(undefined);
		vi.doMock('../services/tabsApi', () => ({
			tabsApi: {
				getSnapshot: async () => ({
					active: 1,
					tabs: [{ id: 1, location: { display: '/old', uri: 'file:///old' } }],
				}),
				navigate,
			},
		}));
		vi.doMock('../services/tauriVfsClient', () => ({
			createTauriVfsClient: () => ({ parseLocation: async () => parsed }),
		}));
		const { openFolder: open } = await import('./perfHarness');
		let done = false;
		const run = open('/a#b').then(() => (done = true));
		await vi.advanceTimersByTimeAsync(500);
		expect(navigate).toHaveBeenCalledWith(1, parsed);
		// The old folder's list (5000 rows) is still on screen: not arrived yet.
		expect(done).toBe(false);
		old.remove();
		const fresh = document.body.appendChild(document.createElement('div'));
		fresh.setAttribute('role', 'listbox');
		fresh.setAttribute('aria-rowcount', '2000');
		await vi.advanceTimersByTimeAsync(200);
		await run;
		expect(done).toBe(true);
		vi.useRealTimers();
	});
});
