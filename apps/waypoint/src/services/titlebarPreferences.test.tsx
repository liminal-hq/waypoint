// Verifies the preferences hook orders snapshots by revision, subscribes before it reads, and tolerates a missing plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TitlebarSnapshot } from '@liminal-hq/plugin-system-appearance';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const plugin = vi.hoisted(() => ({
	read: vi.fn<() => Promise<TitlebarSnapshot>>(),
	subscribe: vi.fn<(cb: (s: TitlebarSnapshot) => void) => Promise<() => void>>(),
}));

vi.mock('@liminal-hq/plugin-system-appearance', () => ({
	getTitlebarPreferences: plugin.read,
	onTitlebarPreferencesChanged: plugin.subscribe,
}));

import { useTitlebarPreferences } from './titlebarPreferences';

const snapshot = (
	revision: number,
	doubleClick: TitlebarSnapshot['actions']['doubleClick'],
): TitlebarSnapshot => ({
	revision,
	buttonLayout: { start: [], end: ['close'] },
	actions: { doubleClick, middleClick: 'none', rightClick: 'menu' },
	desktopEnvironment: 'gnome',
	source: 'portal',
});

beforeEach(() => {
	plugin.read.mockReset();
	plugin.subscribe.mockReset();
});

describe('useTitlebarPreferences', () => {
	it('returns null first, then the reading, then pushed changes', async () => {
		let push: (s: TitlebarSnapshot) => void = () => {};
		plugin.read.mockResolvedValue(snapshot(1, 'toggleMaximise'));
		plugin.subscribe.mockImplementation(async (cb) => {
			push = cb;
			return () => {};
		});

		const { result } = renderHook(() => useTitlebarPreferences());
		expect(result.current).toBeNull();
		await waitFor(() => expect(result.current?.actions.doubleClick).toBe('toggleMaximise'));

		act(() => push(snapshot(2, 'minimise')));
		expect(result.current?.actions.doubleClick).toBe('minimise');
	});

	it('does not let a slower, older read overwrite a newer change event', async () => {
		let push: (s: TitlebarSnapshot) => void = () => {};
		let finishRead: (s: TitlebarSnapshot) => void = () => {};
		plugin.read.mockImplementation(() => new Promise((resolve) => (finishRead = resolve)));
		plugin.subscribe.mockImplementation(async (cb) => {
			push = cb;
			return () => {};
		});

		const { result } = renderHook(() => useTitlebarPreferences());
		await act(async () => {});
		act(() => push(snapshot(2, 'minimise'))); // newer state, delivered first
		await act(async () => finishRead(snapshot(1, 'toggleMaximise'))); // older snapshot lands last
		expect(result.current?.actions.doubleClick).toBe('minimise');
	});

	it('ignores a repeat of a revision it already has', async () => {
		let push: (s: TitlebarSnapshot) => void = () => {};
		plugin.read.mockResolvedValue(snapshot(3, 'lower'));
		plugin.subscribe.mockImplementation(async (cb) => {
			push = cb;
			return () => {};
		});
		const { result } = renderHook(() => useTitlebarPreferences());
		await waitFor(() => expect(result.current?.actions.doubleClick).toBe('lower'));
		act(() => push(snapshot(3, 'none')));
		expect(result.current?.actions.doubleClick).toBe('lower');
	});

	it('subscribes before it reads, so a change in between is not lost', async () => {
		const order: string[] = [];
		plugin.subscribe.mockImplementation(async () => {
			order.push('subscribe');
			return () => {};
		});
		plugin.read.mockImplementation(async () => {
			order.push('read');
			return snapshot(1, 'none');
		});
		renderHook(() => useTitlebarPreferences());
		await act(async () => {});
		expect(order).toEqual(['subscribe', 'read']);
	});

	it('stays null without throwing when the plugin is missing', async () => {
		plugin.subscribe.mockImplementation(() => {
			const rejected = Promise.reject(new Error('no tauri'));
			rejected.catch(() => {});
			return rejected;
		});
		const { result } = renderHook(() => useTitlebarPreferences());
		await act(async () => {});
		expect(result.current).toBeNull();
	});

	it('unsubscribes on unmount', async () => {
		const off = vi.fn();
		plugin.read.mockResolvedValue(snapshot(1, 'none'));
		plugin.subscribe.mockResolvedValue(off);
		const { unmount } = renderHook(() => useTitlebarPreferences());
		await act(async () => {});
		unmount();
		expect(off).toHaveBeenCalledTimes(1);
	});
});
