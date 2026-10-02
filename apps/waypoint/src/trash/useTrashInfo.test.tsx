// Verifies the Trash place's count stays current: at start, on focus, after jobs, on a slow timer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeTrashClient, fakeJobSnapshot } from './fakeTrashClient';
import { TRASH_INFO_INTERVAL_MS, useTrashInfo } from './useTrashInfo';

beforeEach(() => {
	vi.useFakeTimers();
});
afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

const settle = () => act(async () => void (await vi.advanceTimersByTimeAsync(0)));

describe('useTrashInfo', () => {
	it('is null without a Trash service and before the first read', async () => {
		const none = renderHook(() => useTrashInfo(null));
		expect(none.result.current).toBeNull();
		const client = new FakeTrashClient({ count: 4 });
		const { result } = renderHook(() => useTrashInfo(client));
		expect(result.current).toBeNull();
		await settle();
		expect(result.current).toEqual({ available: true, reason: null, count: 4, totalBytes: null });
	});

	it('asks for the total size only when told to, and says so in what it returns', async () => {
		const client = new FakeTrashClient({ count: 2 });
		client.bytes = 4096;
		const plain = renderHook(() => useTrashInfo(client));
		await settle();
		expect(plain.result.current?.totalBytes).toBeNull();
		expect(client.asked).toEqual([false]);
		const measured = renderHook(() => useTrashInfo(client, { withBytes: true }));
		await settle();
		expect(measured.result.current?.totalBytes).toBe(4096);
		expect(client.asked).toEqual([false, true]);
	});

	it('reads again when the window gains focus', async () => {
		const client = new FakeTrashClient({ count: 1 });
		const { result } = renderHook(() => useTrashInfo(client));
		await settle();
		client.setInfo({ count: 2 });
		act(() => window.dispatchEvent(new Event('focus')));
		await settle();
		expect(result.current?.count).toBe(2);
	});

	it('reads again when any job finishes, and only then', async () => {
		const client = new FakeTrashClient({ count: 1 });
		const { result } = renderHook(() => useTrashInfo(client));
		await settle();
		const calls = client.infoCalls;
		client.setInfo({ count: 0 });
		const job = (state: 'running' | 'done') =>
			fakeJobSnapshot(1, { kind: 'trash' }, { state } as never);
		act(() => client.emit({ kind: 'jobChanged', job: job('running'), revision: 2 }));
		await settle();
		expect(client.infoCalls).toBe(calls);
		act(() => client.emit({ kind: 'jobChanged', job: job('done'), revision: 3 }));
		await settle();
		expect(result.current?.count).toBe(0);
	});

	it('reads on a slow timer while the window is visible and not while hidden', async () => {
		const client = new FakeTrashClient({ count: 1 });
		renderHook(() => useTrashInfo(client));
		await settle();
		const calls = client.infoCalls;
		await act(async () => void (await vi.advanceTimersByTimeAsync(TRASH_INFO_INTERVAL_MS)));
		expect(client.infoCalls).toBe(calls + 1);
		vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden');
		await act(async () => void (await vi.advanceTimersByTimeAsync(TRASH_INFO_INTERVAL_MS)));
		expect(client.infoCalls).toBe(calls + 1);
	});

	it('keeps the last answer when a read fails, and stops listening when unmounted', async () => {
		const client = new FakeTrashClient({ count: 3 });
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const { result, unmount } = renderHook(() => useTrashInfo(client));
		await settle();
		client.getInfo = () => Promise.reject(new Error('gone'));
		act(() => window.dispatchEvent(new Event('focus')));
		await settle();
		expect(result.current?.count).toBe(3);
		expect(warn).toHaveBeenCalled();
		unmount();
		const calls = client.infoCalls;
		await act(async () => void (await vi.advanceTimersByTimeAsync(TRASH_INFO_INTERVAL_MS * 2)));
		expect(client.infoCalls).toBe(calls);
	});
});
