// Verifies free space refreshes as a throttle: a burst of changes still reads on a steady beat
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fileLocation } from '../services/fakeVfsClient';
import type { VfsClient } from '../services/vfsClient';
import { FREE_SPACE_REFRESH_MS, useFreeSpace } from './useFreeSpace';

afterEach(() => vi.useRealTimers());

describe('useFreeSpace', () => {
	it('keeps reading during a sustained burst of revisions instead of waiting for quiet', async () => {
		vi.useFakeTimers();
		const getFreeSpace = vi.fn(async () => ({ freeBytes: 1, totalBytes: 2 }));
		const client = { getFreeSpace } as unknown as VfsClient;
		const location = fileLocation('/a');
		const { rerender } = renderHook(({ revision }) => useFreeSpace(client, location, revision), {
			initialProps: { revision: 0 },
		});
		await act(() => vi.advanceTimersByTimeAsync(0));
		expect(getFreeSpace).toHaveBeenCalledTimes(1);

		// A change every 500 ms for 6 s: a debounce would never fire.
		for (let revision = 1; revision <= 12; revision++) {
			rerender({ revision });
			await act(() => vi.advanceTimersByTimeAsync(500));
		}
		expect(getFreeSpace.mock.calls.length).toBeGreaterThanOrEqual(3);
		expect(getFreeSpace.mock.calls.length).toBeLessThanOrEqual(
			1 + Math.ceil(6000 / FREE_SPACE_REFRESH_MS),
		);
	});

	it('reads straight away after a quiet spell, and once more after the burst', async () => {
		vi.useFakeTimers();
		const getFreeSpace = vi.fn(async () => ({ freeBytes: 1, totalBytes: 2 }));
		const client = { getFreeSpace } as unknown as VfsClient;
		const location = fileLocation('/a');
		const { rerender } = renderHook(({ revision }) => useFreeSpace(client, location, revision), {
			initialProps: { revision: 0 },
		});
		await act(() => vi.advanceTimersByTimeAsync(FREE_SPACE_REFRESH_MS + 10));
		rerender({ revision: 1 });
		await act(() => vi.advanceTimersByTimeAsync(0));
		expect(getFreeSpace).toHaveBeenCalledTimes(2);
		rerender({ revision: 2 });
		await act(() => vi.advanceTimersByTimeAsync(0));
		expect(getFreeSpace).toHaveBeenCalledTimes(2);
		await act(() => vi.advanceTimersByTimeAsync(FREE_SPACE_REFRESH_MS));
		expect(getFreeSpace).toHaveBeenCalledTimes(3);
	});
});
