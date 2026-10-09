// Verifies administrator access is offered only while the setting is on and the plugin says it works
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PluginStatus } from '@liminal-hq/plugin-elevate';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS, type Settings } from '../services/settingsClient';
import { SettingsProvider } from '../settings/SettingsContext';
import { ElevationProvider, useElevationAvailable } from './ElevationContext';

afterEach(cleanup);

const WORKS: PluginStatus = {
	available: true,
	reason: null,
	flavour: 'polkit',
	features: [{ name: 'elevate', available: true, reason: null }],
};
const BROKEN: PluginStatus = {
	available: false,
	reason: 'The helper is not installed in a protected folder',
	flavour: 'polkit',
	features: [{ name: 'elevate', available: false, reason: 'not installed' }],
};

const withSetting = (administratorAccess: boolean): Settings => ({
	...DEFAULT_SETTINGS,
	experimental: { ...DEFAULT_SETTINGS.experimental, administratorAccess },
});

function setup(status: () => Promise<PluginStatus>, on: boolean) {
	const client = createFakeSettingsClient(withSetting(on));
	const wrapper = ({ children }: { children: ReactNode }) => (
		<SettingsProvider client={client}>
			<ElevationProvider status={status}>{children}</ElevationProvider>
		</SettingsProvider>
	);
	return { client, ...renderHook(() => useElevationAvailable(), { wrapper }) };
}

describe('useElevationAvailable', () => {
	it('is true when the setting is on and the plugin works', async () => {
		const { result } = setup(() => Promise.resolve(WORKS), true);
		await waitFor(() => expect(result.current).toBe(true));
	});

	it('is false, and never asks the plugin, while the setting is off', async () => {
		const status = vi.fn(() => Promise.resolve(WORKS));
		const { result } = setup(status, false);
		await act(async () => {});
		expect(result.current).toBe(false);
		expect(status).not.toHaveBeenCalled();
	});

	it('is false when the plugin says it cannot start a helper', async () => {
		const { result } = setup(() => Promise.resolve(BROKEN), true);
		await act(async () => {});
		expect(result.current).toBe(false);
	});

	it('is false when the status cannot be read', async () => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const { result } = setup(() => Promise.reject(new Error('no plugin')), true);
		await act(async () => {});
		expect(result.current).toBe(false);
		warn.mockRestore();
	});

	it('follows the setting as it is turned on and off', async () => {
		const status = vi.fn(() => Promise.resolve(WORKS));
		const { result, client } = setup(status, false);
		expect(result.current).toBe(false);
		await act(async () => {
			client.change(withSetting(true));
		});
		await waitFor(() => expect(result.current).toBe(true));
		expect(status).toHaveBeenCalledTimes(1);
		await act(async () => {
			client.change(withSetting(false));
		});
		await waitFor(() => expect(result.current).toBe(false));
	});
});
