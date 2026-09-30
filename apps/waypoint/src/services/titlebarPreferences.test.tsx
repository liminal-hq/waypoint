// Verifies the preferences hook reads once, follows pushed changes, and tolerates a missing plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TitlebarPreferences } from '@liminal-hq/plugin-system-appearance';
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const plugin = vi.hoisted(() => ({
	read: vi.fn<() => Promise<TitlebarPreferences>>(),
	subscribe: vi.fn<(cb: (p: TitlebarPreferences) => void) => Promise<() => void>>(),
}));

vi.mock('@liminal-hq/plugin-system-appearance', () => ({
	getTitlebarPreferences: plugin.read,
	onTitlebarPreferencesChanged: plugin.subscribe,
}));

import { useTitlebarPreferences } from './titlebarPreferences';

const preferences = (
	doubleClick: TitlebarPreferences['actions']['doubleClick'],
): TitlebarPreferences => ({
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
		let push: (p: TitlebarPreferences) => void = () => {};
		plugin.read.mockResolvedValue(preferences('toggleMaximise'));
		plugin.subscribe.mockImplementation(async (cb) => {
			push = cb;
			return () => {};
		});

		const { result } = renderHook(() => useTitlebarPreferences());
		expect(result.current).toBeNull();
		await waitFor(() => expect(result.current?.actions.doubleClick).toBe('toggleMaximise'));

		act(() => push(preferences('minimise')));
		expect(result.current?.actions.doubleClick).toBe('minimise');
	});

	it('stays null without throwing when the plugin is missing', async () => {
		plugin.read.mockRejectedValue(new Error('no tauri'));
		plugin.subscribe.mockRejectedValue(new Error('no tauri'));
		const { result } = renderHook(() => useTitlebarPreferences());
		await act(async () => {});
		expect(result.current).toBeNull();
	});

	it('unsubscribes on unmount', async () => {
		const off = vi.fn();
		plugin.read.mockResolvedValue(preferences('none'));
		plugin.subscribe.mockResolvedValue(off);
		const { unmount } = renderHook(() => useTitlebarPreferences());
		await act(async () => {});
		unmount();
		expect(off).toHaveBeenCalledTimes(1);
	});
});
